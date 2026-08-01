pub use stm32l4xx_hal as hal_impl;
pub use stm32l4xx_hal::pac;

use crate::mcu::ChipConfig;

pub struct Chip;

impl ChipConfig for Chip {
    const CPU_FREQUENCY_MHZ: u32 = 80;
    const EEPROM_START: u32 = 0x0800_F800;
    const FLASH_PAGE_SIZE: u32 = 0x800;
    const TIMER_PSC: u16 = 39;
    const GCR_SHIFT: u8 = 7;
    const COMP_EXTI_LINE: u32 = 22;
    const INPUT_DMA_CHANNEL: usize = 4;
    const ADC_CURRENT_CHANNEL: u8 = 8;
    const ADC_VOLTAGE_CHANNEL: u8 = 11;
    const WDG_PRESCALER: u8 = 2;
    const WDG_RELOAD: u16 = 4000;
}
pub use super::flash::FlashStorage;

/// Enable TIM2 peripheral clock.
pub fn enable_tim2_clock() {
    // SAFETY: Single-core MCU, called during init before interrupts are enabled.
    // RCC is a single-owner peripheral; no concurrent access.
    let rcc = unsafe { &*pac::RCC::PTR };
    rcc.apb1enr1
        // SAFETY: Setting only the TIM2EN bit (bit 0); other bits preserved via read-modify-write.
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 0)) }); // TIM2EN
}

/// Enable commutation timer (TIM16) peripheral clock.
pub fn enable_com_timer_clock() {
    // SAFETY: Single-core MCU, called during init before interrupts are enabled.
    // RCC is a single-owner peripheral; no concurrent access.
    let rcc = unsafe { &*pac::RCC::PTR };
    rcc.apb2enr
        // SAFETY: Setting only the TIM16EN bit (bit 17); other bits preserved via read-modify-write.
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 17)) }); // TIM16EN
}

/// Adjust IRQ priorities based on motor speed.
/// Low eRPM: DShot DMA > commutation (don't drop input frames)
/// High eRPM: commutation > DShot (don't miss commutation steps)
///
/// NOTE: STM32L4 NVIC has 4 priority bits in the UPPER nibble of each IPR
/// byte. cortex_m's `set_priority` writes the raw byte, so priority levels
/// must be passed as `level << 4`. Prior versions of this function passed
/// raw 0/1 which both collapsed to level 0 — the priority swap was a no-op.
pub fn adjust_irq_priorities(interval: u32, dshot_telem: bool) {
    use pac::Interrupt;
    const DSHOT_PRIORITY_THRESHOLD: u32 = 60;
    // SAFETY: NVIC is a core peripheral with a fixed address. We are the only
    // code adjusting these priorities; the main loop calls this outside of ISR context.
    let nvic =
        unsafe { &mut *(cortex_m::peripheral::NVIC::PTR as *mut cortex_m::peripheral::NVIC) };
    if dshot_telem && interval > DSHOT_PRIORITY_THRESHOLD {
        // SAFETY: Setting valid priority values (level 0-1, shifted into the
        // top 4 bits of the priority byte). Atomic per-IRQ in the NVIC.
        unsafe {
            nvic.set_priority(Interrupt::DMA1_CH5, 0 << 4);
            nvic.set_priority(Interrupt::TIM1_UP_TIM16, 1 << 4);
            nvic.set_priority(Interrupt::COMP, 1 << 4);
        }
    } else {
        // SAFETY: Same as above — valid shifted priority levels.
        unsafe {
            nvic.set_priority(Interrupt::DMA1_CH5, 1 << 4);
            nvic.set_priority(Interrupt::TIM1_UP_TIM16, 0 << 4);
            nvic.set_priority(Interrupt::COMP, 0 << 4);
        }
    }
}

pub type TargetIsrHal = crate::isr::IsrHal<
    super::pwm::Pwm,
    super::input_capture::L431DshotCapture,
    super::comparator::L431BemfComparator,
    crate::timer::Tim2Interval,
    crate::timer::Tim14Com,
    crate::phase::G0APhaseDriver,
>;
pub use super::comparator::L431BemfComparator as BemfComp;
pub use super::init::init as init_mcu;
/// Direct TIM1 CCR write for sine PWM — no ISR state needed, safe from main.
pub fn write_tim1_ccr(ch1: u16, ch2: u16, ch3: u16) {
    let tim1 = unsafe { &*pac::TIM1::ptr() };
    tim1.ccr1.write(|w| unsafe { w.bits(ch1 as u32) });
    tim1.ccr2.write(|w| unsafe { w.bits(ch2 as u32) });
    tim1.ccr3.write(|w| unsafe { w.bits(ch3 as u32) });
}

crate::define_port!(field, PortA, crate::pac::GPIOA);
crate::define_port!(field, PortB, crate::pac::GPIOB);
crate::define_raw_timer!(field, Tim2Raw, crate::pac::TIM2);
crate::define_raw_timer!(field, ComTimerRaw, crate::pac::TIM16);
