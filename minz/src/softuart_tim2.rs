//! `BitSampleTimer` impl for stm32l4xx-hal's `Timer<TIM2>`, delegating to
//! [`crate::timer_ext::Tim2Ctrl`] for the pause/resume/CNT/flag operations.
//!
//! Lives here (not in the binary) because Rust's orphan rule requires a
//! trait impl for a foreign type to be in the crate defining the trait —
//! `BitSampleTimer` is defined in [`crate::softuart`] which is part of
//! `minz`. The file is kept separate from `softuart.rs` so the latter stays
//! pure state machine + decoder, and per-timer adapters can be added by
//! creating sibling files (`softuart_tim3.rs`, etc.) without touching the
//! generic core.
//!
//! The `Tim2Ctrl` trait is imported here so its `pause`/`resume`/`set_cnt`/
//! `clear_all_flags` method names don't collide with `BitSampleTimer`'s at
//! the binary call sites — the local import only affects this file.

use crate::hal::pac::TIM2;
use crate::hal::timer::Timer;
use crate::softuart::BitSampleTimer;
use crate::timer_ext::Tim2Ctrl;

impl BitSampleTimer for Timer<TIM2> {
    fn reload(&mut self, cnt: u32) {
        Tim2Ctrl::pause(self);
        Tim2Ctrl::set_cnt(self, cnt);
        Tim2Ctrl::clear_all_flags(self);
        Tim2Ctrl::resume(self);
    }
    fn pause(&mut self) {
        Tim2Ctrl::pause(self)
    }
    fn clear_update_interrupt_flag(&mut self) {
        Timer::<TIM2>::clear_update_interrupt_flag(self)
    }
}
