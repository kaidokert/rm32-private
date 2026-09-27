//! Execution policy for the bounded persistence-to-arm transaction.
//! Default stays interruptible; the separate candidate is not motor-qualified.

pub mod check;

pub trait Window {
    const MASKED: bool;
    const AFTER_FILTER: bool = false;
    fn run<T>(f: impl FnOnce() -> T) -> T;
}

pub struct CommitMasked;
impl Window for CommitMasked {
    const MASKED: bool = true;
    const AFTER_FILTER: bool = true;
    #[inline(always)]
    fn run<T>(f: impl FnOnce() -> T) -> T {
        cortex_m::interrupt::free(|_| f())
    }
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
