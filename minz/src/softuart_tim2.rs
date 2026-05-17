//! `IrqAck` impl for stm32l4xx-hal's `Timer<TIM2>`.
//!
//! Lives here (not in the binary) because of Rust's orphan rule: the
//! trait is defined in [`crate::softuart`] which is part of `minz`, so
//! the impl must also be in `minz`.

use crate::hal::pac::TIM2;
use crate::hal::timer::Timer;
use crate::softuart::IrqAck;

impl IrqAck for Timer<TIM2> {
    fn ack(&mut self) {
        // HAL inherent on `Timer<TIM2>` — clears TIM2_SR.UIF.
        Timer::<TIM2>::clear_update_interrupt_flag(self)
    }
}
