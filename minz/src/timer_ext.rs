//! `Timer<TIM2>` helpers for bit-bang UART.
//!
//! Register writes mirror vendored HAL (under `minz/target/vendor/` after `cargo vendor`):
//!
//! | What | Path |
//! |------|------|
//! | `pause` / `resume` | `stm32l4xx-hal/src/timer.rs` — `free()`, `start()` |
//! | `listen` / `clear_interrupt` | same file — used directly on `Timer` |
//! | `Mutex` + ISR borrow | `stm32l4xx-hal/examples/irq_button.rs` |
//! | periodic timer + `Event::TimeOut` | `stm32l4xx-hal/examples/timer.rs` |
//!
//! Upstream `Timer` does not expose `set_arr` / `set_cnt` / `pause` while the wrapper is alive.

use crate::hal::pac::TIM2;
use crate::hal::timer::Timer;

#[inline]
fn tim2() -> &'static crate::hal::pac::tim2::RegisterBlock {
    unsafe { &*TIM2::ptr() }
}

/// Dynamic ARR/CNT and pause/resume while keeping the HAL `Timer` wrapper in a static.
pub trait Tim2Ctrl {
    fn pause(&mut self);
    fn resume(&mut self);
    fn set_arr(&mut self, arr: u32);
    fn set_cnt(&mut self, cnt: u32);
    fn clear_all_flags(&mut self);
}

impl Tim2Ctrl for Timer<TIM2> {
    #[inline]
    fn pause(&mut self) {
        tim2().cr1.modify(|_, w| w.cen().clear_bit());
    }

    #[inline]
    fn resume(&mut self) {
        tim2().cr1.modify(|_, w| w.cen().set_bit());
    }

    #[inline]
    fn set_arr(&mut self, arr: u32) {
        tim2().arr.write(|w| w.arr().bits(arr));
    }

    #[inline]
    fn set_cnt(&mut self, cnt: u32) {
        tim2().cnt.write(|w| w.cnt().bits(cnt));
    }

    #[inline]
    fn clear_all_flags(&mut self) {
        tim2().sr.write(|w| unsafe { w.bits(0) });
    }
}
