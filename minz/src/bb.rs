//! Platform wrapper for the minz_core black box: a critical-section
//! cell + timestamped record. The static instance lives with the
//! program (am32_clone.rs); every user takes `&Bb` as a parameter.

use core::cell::RefCell;
use cortex_m::interrupt::{Mutex, free};
use minz_core::blackbox::{BlackBox, Event, format_dump};

pub struct Bb(pub Mutex<RefCell<BlackBox>>);

impl Bb {
    pub const fn new() -> Self {
        Bb(Mutex::new(RefCell::new(BlackBox::new())))
    }

    /// bb_record: timestamp from `am32_timers::now_10us`.
    #[inline]
    pub fn record(&self, ty: u8, sector: u8, data: u16) {
        let t = crate::am32_timers::now_10us();
        free(|cs| self.0.borrow(cs).borrow_mut().record(Event { t, ty, sector, data }));
    }

    /// Freeze the ring (a fault freezes so the dump shows the events
    /// LEADING TO the kill, not the aftermath).
    #[inline]
    pub fn freeze(&self) {
        free(|cs| self.0.borrow(cs).borrow_mut().freeze());
    }

    /// Replay under the lock into a byte sink (dump_bb keeps its shape).
    pub fn dump(&self, mut sink: impl FnMut(&[u8])) {
        free(|cs| {
            let bb = self.0.borrow(cs).borrow();
            format_dump(bb.replay().copied(), &mut sink);
        });
    }
}

impl Default for Bb {
    fn default() -> Self {
        Self::new()
    }
}

/// `minz_core::am32_hal::Recorder` over the black box — the trait
/// methods delegate to the inherent ones above (inherent methods win
/// resolution, so `self.record(..)` below is NOT recursive).
impl minz_core::am32_hal::Recorder for Bb {
    #[inline(always)]
    fn record(&self, ty: u8, sector: u8, data: u16) {
        self.record(ty, sector, data)
    }
    #[inline(always)]
    fn freeze(&self) {
        self.freeze()
    }
}

/// `minz_core::am32_hal::Cs` via `cortex_m::interrupt::free` — the
/// platform critical-section provider (zero-sized, static dispatch).
pub struct CortexCs;

impl minz_core::am32_hal::Cs for CortexCs {
    #[inline(always)]
    fn free<R>(&self, f: impl FnOnce() -> R) -> R {
        free(|_| f())
    }
}
