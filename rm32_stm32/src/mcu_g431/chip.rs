pub use stm32g4xx_hal as hal_impl;
pub use stm32g4xx_hal::stm32 as pac;

use crate::mcu::ChipConfig;

pub struct Chip;

impl ChipConfig for Chip {
    const CPU_FREQUENCY_MHZ: u32 = 170;
    const EEPROM_START: u32 = 0x0800_F800;
    const FLASH_PAGE_SIZE: u32 = 0x800;
    const TIMER_PSC: u16 = 84;
    const GCR_SHIFT: u8 = 7;
    const COMP_EXTI_LINE: u32 = 21;
    const INPUT_DMA_CHANNEL: usize = 0;
    const ADC_CURRENT_CHANNEL: u8 = 5;
    const ADC_VOLTAGE_CHANNEL: u8 = 13;
    const WDG_PRESCALER: u8 = 2;
    const WDG_RELOAD: u16 = 4000;
}
pub use super::flash::FlashStorage;

/// Enable TIM2 peripheral clock.
pub fn enable_tim2_clock() {
    // SAFETY: Single-core MCU, called during init before interrupts are enabled.
    // RCC is a single-owner peripheral; no concurrent access.
    let rcc = unsafe { &*pac::RCC::PTR };
    rcc.apb1enr1().modify(|_, w| w.tim2en().set_bit()); // TIM2EN
}

/// Enable commutation timer (TIM16) peripheral clock.
pub fn enable_com_timer_clock() {
    // SAFETY: Single-core MCU, called during init before interrupts are enabled.
    // RCC is a single-owner peripheral; no concurrent access.
    let rcc = unsafe { &*pac::RCC::PTR };
    rcc.apb2enr().modify(|_, w| w.tim16en().set_bit()); // TIM16EN
}

/// Adjust IRQ priorities based on motor speed. No-op on G431.
pub fn adjust_irq_priorities(_interval: u32, _dshot_telem: bool) {}

pub type TargetIsrHal = crate::isr::IsrHal<
    super::pwm::Pwm,
    super::input_capture::G431DshotCapture,
    super::comparator::G431BemfComparator,
    crate::timer::Tim2Interval,
    crate::timer::Tim14Com,
    crate::phase::G0APhaseDriver,
>;
pub use super::comparator::G431BemfComparator as BemfComp;
pub use super::init::init as init_mcu;

crate::define_port!(method, PortA, crate::pac::GPIOA);
crate::define_port!(method, PortB, crate::pac::GPIOB);
crate::define_raw_timer!(method, Tim2Raw, crate::pac::TIM2);
crate::define_raw_timer!(method, ComTimerRaw, crate::pac::TIM16);
/// Direct TIM1 CCR write for sine PWM — no ISR state needed, safe from main.
pub fn write_tim1_ccr(ch1: u16, ch2: u16, ch3: u16) {
    use stm32g4xx_hal::stm32 as pac;
    let tim1 = unsafe { &*pac::TIM1::PTR };
    unsafe {
        tim1.ccr1().write(|w| w.ccr().bits(ch1 as u32));
        tim1.ccr2().write(|w| w.ccr().bits(ch2 as u32));
        tim1.ccr3().write(|w| w.ccr().bits(ch3 as u32));
    }
}
