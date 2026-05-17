//! `IrqAck` impl for stm32l4xx-hal's `LowPowerTimer<LPTIM1>`.
//!
//! Clears the CompareMatch event flag. Lives here (not in the binary)
//! because of Rust's orphan rule: the trait is defined in
//! [`crate::softuart`].

use crate::hal::lptimer::{Event, LowPowerTimer};
use crate::hal::pac::LPTIM1;
use crate::softuart::IrqAck;

impl IrqAck for LowPowerTimer<LPTIM1> {
    fn ack(&mut self) {
        self.clear_event_flag(Event::CompareMatch);
    }
}
