//! USART2 receiver on PA2 (the J3 `S` pin) via CR2.SWAP — raw
//! register init because the HAL's `Serial::usart2` insists on PA3
//! for RX (no swap support), and PA3 is the current-sense ADC input.
//!
//! PA2's AF7 function is USART2_TX; SWAP routes the receiver onto
//! it. TE stays 0: receive-only, we never drive the pin (the GPIO
//! should also be open-drain so a mistake can't fight the host
//! adapter). Kernel clock is the CCIPR reset default (PCLK1).

use crate::hal::stm32;

/// One-time init at `baud` 8N1, RX + RXNE interrupt enabled. Per
/// RM0394, BRR and CR2.SWAP are only writable while UE=0 (true out
/// of reset — call this once, before any other USART2 touch).
/// The caller must have configured PA2 as AF7 (open-drain, pull-up)
/// and owns the NVIC unmask + priority.
pub fn init_pa2_rx(usart2: stm32::USART2, pclk1_hz: u32, baud: u32) {
    unsafe {
        (*stm32::RCC::ptr())
            .apb1enr1
            .modify(|_, w| w.usart2en().set_bit());
    }
    usart2.cr2.write(|w| w.swap().set_bit());
    usart2
        .brr
        .write(|w| unsafe { w.bits((pclk1_hz + baud / 2) / baud) });
    usart2
        .cr1
        .write(|w| w.re().set_bit().rxneie().set_bit().ue().set_bit());
}
