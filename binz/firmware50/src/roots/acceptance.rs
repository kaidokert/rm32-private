//! Execution policy for the bounded persistence-to-arm transaction.
//! Default stays interruptible; the separate candidate is not motor-qualified.

pub trait Window {
    const MASKED: bool;
    fn run<T>(f: impl FnOnce() -> T) -> T;
}

pub struct Open;
impl Window for Open {
    const MASKED: bool = false;
    #[inline(always)]
    fn run<T>(f: impl FnOnce() -> T) -> T { f() }
}

pub struct Masked;
impl Window for Masked {
    const MASKED: bool = true;
    #[inline(always)]
    fn run<T>(f: impl FnOnce() -> T) -> T {
        // Restores the incoming PRIMASK, including nested arm critical sections.
        // The caller must return before statistics/watch/trace bookkeeping.
        cortex_m::interrupt::free(|_| f())
    }
}
