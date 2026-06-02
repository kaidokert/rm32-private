// Dependencies are referenced through $crate:: in the macro

/// Generic macro to define parameter structures for algorithms.
///
/// This macro generates three types for any parameter set:
/// - `[Name]Snap<T>`: Snapshot struct with public fields
/// - `[Name]Plain<T>`: Cell-based implementation for single-threaded use
/// - `[Name]Atomic<T>`: Lock-free atomic implementation for multi-threaded use
/// - `[Name]Provider<T>`: Trait with snapshot and setter methods
///
/// # Example
/// ```
/// use rinz::define_params;
///
/// define_params!(PIParams {
///     kp: T,
///     ki: T,
///     min: T,
///     max: T
/// });
/// ```
///
/// This generates:
/// - `PIParamsSnap<T>` - snapshot with fields `kp`, `ki`, `min`, `max`
/// - `PIParamsPlain<T>` - cell-based storage
/// - `PIParamsAtomic<T>` - atomic storage with lock-free consistency
/// - `PIParamsProvider<T>` - trait with `with_snapshot`, `set_kp`, `set_ki`, `set_min`, `set_max`
#[macro_export]
macro_rules! define_params {
    ( $name:ident { $( $field:ident : $ty:ty ),+ $(,)? } ) => {
        paste::paste! {
            #[derive(Copy, Clone)]
            pub struct [<$name Snap>]<T> { $( pub $field: T ),+ }

            pub trait [<$name Provider>]<T: Copy> {
                fn with_snapshot<R>(&self, f: impl FnOnce([<$name Snap>]<T>) -> R) -> R;
                $( fn [<set_ $field>](&self, v: T); )+
                // NOTE: Temporary
                $( fn [<get_ $field>](&self) -> T; )+
            }

            // -------- Plain (Cell) --------
            pub struct [<$name Plain>]<T: Copy> { $( $field: core::cell::Cell<T> ),+ }

            impl<T: Copy> [<$name Plain>]<T> {
                pub const fn new( $( $field: T ),+ ) -> Self {
                    Self { $( $field: core::cell::Cell::new($field) ),+ }
                }
            }

            impl<T: Copy> [<$name Provider>]<T> for [<$name Plain>]<T> {
                fn with_snapshot<R>(&self, f: impl FnOnce([<$name Snap>]<T>) -> R) -> R {
                    f([<$name Snap>] { $( $field: self.$field.get() ),+ })
                }
                $( fn [<set_ $field>](&self, v: T) { self.$field.set(v); } )+
                // NOTE: Temporary
                $( fn [<get_ $field>](&self) -> T { self.$field.get() } )+
            }

            // -------- Atomic (coherent) --------
            pub struct [<$name Atomic>]<T: Copy + bytemuck::NoUninit> {
                $( $field: atomic::Atomic<T> ),+,
                ver: core::sync::atomic::AtomicU32,
            }

            impl<T: Copy + bytemuck::NoUninit> [<$name Atomic>]<T> {
                pub const fn new( $( $field: T ),+ ) -> Self {
                    // This will panic at runtime if u64 atomics aren't supported, but that's better than silent corruption
                    Self { $( $field: atomic::Atomic::new($field) ),+, ver: core::sync::atomic::AtomicU32::new(0) }
                }

                #[inline]
                fn load_snap(&self) -> [<$name Snap>]<T> {
                    use core::sync::atomic::Ordering;
                    [<$name Snap>] { $( $field: self.$field.load(Ordering::Acquire) ),+ }
                }

                /// Single writer transaction via snapshot transform.
                #[inline]
                fn update_with(&self, f: impl FnOnce([<$name Snap>]<T>) -> [<$name Snap>]<T>) {
                    use core::sync::atomic::Ordering;
                    self.ver.fetch_add(1, Ordering::Relaxed); // odd (publish start)
                    let cur = self.load_snap();
                    let new = f(cur);
                    $( self.$field.store(new.$field, Ordering::Release); )+
                    self.ver.fetch_add(1, Ordering::Release); // even (publish end)
                }
            }

            impl<T: Copy + bytemuck::NoUninit> [<$name Provider>]<T> for [<$name Atomic>]<T> {
                fn with_snapshot<R>(&self, f: impl FnOnce([<$name Snap>]<T>) -> R) -> R {
                    use core::sync::atomic::Ordering;
                    loop {
                        let v1 = self.ver.load(Ordering::Acquire);
                        if v1 & 1 != 0 { continue; } // writer in progress
                        let snap = self.load_snap();
                        let v2 = self.ver.load(Ordering::Acquire);
                        if v1 == v2 { return f(snap); }
                    }
                }
                $(
                    fn [<set_ $field>](&self, v: T) {
                        self.update_with(|mut s| { s.$field = v; s })
                    }
                )+
                // NOTE: Temporary
                $(
                    fn [<get_ $field>](&self) -> T {
                        self.$field.load(core::sync::atomic::Ordering::Acquire)
                    }
                )+
            }
        }
    };
}

// Re-export the macro for external use
pub use define_params;
