use crate::*;

/// Struct used to run a system function using the world.
/// This struct is also used internally by the `Dispatcher` to create a coherent
/// execution sequence.
pub struct System {
    pub initialize: Box<dyn Fn(&mut World) + Send>,
    pub lock: Box<dyn Fn(*const World, *mut Vec<Box<dyn RefLifetime>>) -> SystemResult + Send>,
    pub run_fn: Box<dyn FnMut(&World) -> SystemResult + Send>,
    pub name: &'static str,
}

impl System {
    /// Initializes the resources required to run this system inside of the
    /// provided `World`, if those resources don't already exist.
    ///
    /// This is called automatically if you use a `Dispatcher`, so in most
    /// cases it is not required to call it manually.
    pub fn initialize(&self, world: &mut World) {
        (self.initialize)(world)
    }
    /// Runs the system's function using the provided `World`'s resources.
    pub fn run(&mut self, world: &World) -> SystemResult {
        (self.run_fn)(world)
    }

    /// Returns the underlying type name of the system. This is not guranteed to
    /// be stable or human-readable, but can be used for diagnostics.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// Converts a function into a `System`. It is required to execute a function
/// automatically from `World`'s resources.
/// This trait is automatically implemented for functions taking 12 arguments (22 if using the
/// `big_systems` feature)
/// or less where:
/// - All arguments are immutable or mutable references.
/// - All immutable references are placed *before* all mutable references.
/// - All arguments implement `Default`.
/// - Does not use the same type twice.
/// - Returns a `SystemResult` (usually just `Ok(())`).
pub trait IntoSystem<R> {
    fn system(self) -> System;
}

macro_rules! impl_system {
    ($($id:ident,)* $(&mut $idmut:ident,)*) => {
        impl<$($id: Send + Sync,)* $($idmut: Send + Sync,)* F> IntoSystem<($(&$id,)* $(&mut $idmut,)*)> for F
        where
            $($id: Default+'static,)*
            $($idmut: Default+'static,)*
            F: Fn($(&$id,)* $(&mut $idmut,)*) -> SystemResult + 'static + Send,
        {
            fn system(self) -> System {
                System {
                    initialize: Box::new(|_world: &mut World| {
                        $(_world.initialize::<$id>();)*
                        $(_world.initialize::<$idmut>();)*
                    }),
                    lock: Box::new(|_world: *const World, _locked: *mut Vec<Box<dyn RefLifetime>>| {
                        // Unsafe: used to extend the lifetime because we need to store the
                        // reference of a value that is inside a RefCell to keep the counter
                        // incremented.
                        $(unsafe {(&mut *_locked).push(Box::new((*_world).get::<$id>()?))};)*
                        $(unsafe {(&mut *_locked).push(Box::new((*_world).get_mut::<$idmut>()?))};)*
                        Ok(())
                    }),
                    run_fn: Box::new(move |_world: &World| {
                        self($(&*_world.get::<$id>()?,)* $(&mut *_world.get_mut::<$idmut>()?),*)
                    }),
                    name: std::any::type_name::<F>()
                }
            }
        }
    }
}

macro_rules! impl_system_muts {
    ($($processed:ident),*$(,)?;) => {
        impl_system!($(&mut $processed,)*);
    };
    ($($processed:ident),*$(,)?; $head:ident, $($tail:ident,)*) => {
        impl_system!($($tail,)* $head, $(&mut $processed,)*);
        impl_system_muts!($($processed,)* $head; $($tail,)*);
    }
}
macro_rules! impl_systems {
    // base case
    () => {};
    ($head:ident, $($idents:ident,)*) => {
        // recursive call
        impl_system_muts!(; $head, $($idents,)*);
        impl_systems!($($idents,)*);
    }
}

impl_system!();
#[cfg(not(feature = "big_systems"))]
impl_systems!(A, B, C, D, E, G, H, I, J, K, L, M,);
// Sometimes I just hate rust. This compiles *very* slowly.
#[cfg(feature = "big_systems")]
// 16
//impl_systems!(A, B, C, D, E, G, H, I, J, K, L, M, O, P, Q, R,);
// 26, 17s build time
//impl_systems!(A, B, C, D, E, G, H, I, J, K, L, M, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA,);
// 22, 10s build time
impl_systems!(A, B, C, D, E, G, H, I, J, K, L, M, O, P, Q, R, S, T, U, V, W,);

#[cfg(test)]
mod tests {
    use crate::*;
    use wasm_bindgen_test::*;

    #[test]
    #[wasm_bindgen_test]
    fn convert_system() {
        let _ = generic::<u32>.system();
        fn tmp(_var1: &u32, _var2: &u64, _var3: &mut i32, _var4: &mut i64) -> SystemResult {
            Ok(())
        }
        // Technically reusing the same type is incorrect and causes a runtime panic.
        // However, there doesn't seem to be a clean way to handle type inequality in generics.
        fn tmp2(
            _var1: &u32,
            _var2: &u64,
            _var3: &mut i32,
            _var4: &mut i64,
            _var5: &mut i64,
            _var6: &mut i64,
            _var7: &mut i64,
            _var8: &mut i64,
            _var9: &mut i64,
            _var10: &mut i64,
            _var11: &mut i64,
            _var12: &mut i64,
        ) -> SystemResult {
            Ok(())
        }
        let _ = tmp.system();
        let _ = tmp2.system();
    }

    #[test]
    #[wasm_bindgen_test]
    fn system_is_send() {
        let x = 6;
        send(
            (move |_var1: &u32| {
                let _y = x;
                Ok(())
            })
            .system(),
        );
        send((|| Ok(())).system());
        send(sys.system());
    }

    fn sys(_var1: &u32) -> SystemResult {
        Ok(())
    }
    fn generic<T>(_t: &T) -> SystemResult {
        Ok(())
    }
    fn send<T: Send>(_t: T) {}

    #[test]
    #[wasm_bindgen_test]
    fn manual_system_run() {
        let mut world = World::default();
        world.initialize::<u32>();
        generic::<u32>.system().run(&world).unwrap();
    }

    #[test]
    #[wasm_bindgen_test]
    fn system_replace_resource() {
        #[derive(Default)]
        pub struct A;
        #[derive(Default)]
        pub struct B {
            x: u32,
        }
        let mut world = World::default();
        let mut my_system = (|_a: &A, b: &mut B| {
            let b2 = B { x: 45 };
            *b = b2;
            Ok(())
        })
        .system();
        my_system.initialize(&mut world);
        my_system.run(&world).unwrap();
        assert_eq!(world.get::<B>().unwrap().x, 45);
    }
}
