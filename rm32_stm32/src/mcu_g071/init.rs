//! STM32G071 initialization.

use crate::init::InitResult;
use crate::phase::G0APhaseDriver;
use crate::timer::{Tim2Interval, Tim14Com};

pub fn init(
    dead_time: u8,
    bemf_pins: rm32::board::BemfPins,
) -> InitResult<super::system::SystemControl, super::adc::AdcReader, super::telemetry_uart::TelemUart>
{
    use stm32g0xx_hal::prelude::*;
    use stm32g0xx_hal::rcc::Config as RccConfig;
    use stm32g0xx_hal::stm32;
    use stm32g0xx_hal::time::Hertz;

    let dp = stm32::Peripherals::take().unwrap();
    let _cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(RccConfig::pll());
    // Flash prefetch buffer ON — AM32 Mcu/g071/Src/peripherals.c:29
    // (`FLASH->ACR |= FLASH_ACR_PRFTEN`). rm32 never set it, so at 64 MHz /
    // 2 wait states every sequential line fetch and branch target paid the
    // full latency: the 20 kHz tick alone averaged 1610 cycles (25 us) idle
    // and overran ~37 % of ticks at 30 % duty, starving the main loop into
    // IWDG resets above ~80 % (binz). Read-modify-write ONLY: ACR bit 18
    // (DBG_SWEN) resets to 1 and a wholesale write disables SWD.
    unsafe {
        let flash = &*stm32::FLASH::ptr();
        flash.acr().modify(|r, w| w.bits(r.bits() | (1 << 8))); // PRFTEN
    }
    let gpioa = dp.GPIOA.split(&mut rcc);
    let _gpiob = dp.GPIOB.split(&mut rcc);

    let pwm = super::pwm::Tim1Pwm::new(
        dp.TIM1,
        gpioa.pa8,
        gpioa.pa9,
        gpioa.pa10,
        Hertz::from_raw(24_000),
        &mut rcc,
        dead_time,
    );
    let phase = G0APhaseDriver::new(false);
    let sys = super::system::SystemControl::new(dp.IWDG);
    super::comp_init::init_comp2();
    let comp = super::comparator::new_comparator(bemf_pins);
    let interval = Tim2Interval::new();
    let com_timer = Tim14Com::new();

    // DShot input capture
    super::input_capture::init_g071();
    let mut input = super::input_capture::new_capture();
    use rm32::hal::InputCapture;
    input.receive_dshot_dma();

    let adc = super::adc::new_adc();
    if let Err(e) = adc.init() {
        let (kind, what) = e.parts();
        crate::dprintln!("[rm32] ADC init FAILED: {} {}", kind, what);
    }
    let telem = super::telemetry_uart::TelemUart::init()
        .unwrap_or_else(|_| super::telemetry_uart::TelemUart::post_init());

    // TIM6: 20kHz
    {
        let rcc_raw = unsafe { &*stm32::RCC::ptr() };
        rcc_raw.apbenr1().modify(|_, w| w.tim6en().set_bit());
        let tim6 = unsafe { &*stm32::TIM6::ptr() };
        tim6.psc().write(|w| unsafe { w.bits(0) });
        tim6.arr().write(|w| unsafe { w.bits(3199) });
        tim6.egr().write(|w| w.ug().set_bit());
        tim6.sr().write(|w| w.uif().clear_bit());
        tim6.dier().write(|w| w.uie().set_bit());
        tim6.cr1().write(|w| w.cen().set_bit());
    }
    // Three-shunt current: one hardware-triggered scan per TIM6 update.
    #[cfg(rm32_three_shunt)]
    super::adc::arm_hw_trigger();

    // NVIC priorities — AM32 G071-matched (Mcu/g071/Src/peripherals.c:
    // COMP 0, COM timer 0, input DMA 1, tenKhzRoutine TIM6 2, EXTI4_15 2).
    // Cortex-M0+ implements 2 priority bits in the TOP of each IPR byte;
    // cortex_m's `set_priority` writes the raw byte, so level N is
    // `N << 6` (CMSIS NVIC_SetPriority shifts internally). Previously
    // unset (all level 0): TIM6's control tick could not be preempted by
    // COMP/commutation — the same defect class as the L431 priority fix.
    unsafe {
        use stm32::{Interrupt, NVIC};
        let mut nvic = cortex_m::Peripherals::steal().NVIC;
        nvic.set_priority(Interrupt::ADC_COMP, 0 << 6);
        nvic.set_priority(Interrupt::TIM14, 0 << 6);
        nvic.set_priority(Interrupt::DMA1_CHANNEL1, 1 << 6);
        nvic.set_priority(Interrupt::TIM6_DAC_LPTIM1, 2 << 6);
        nvic.set_priority(Interrupt::EXTI4_15, 2 << 6);
        NVIC::unmask(Interrupt::TIM6_DAC_LPTIM1);
        NVIC::unmask(Interrupt::TIM14);
        NVIC::unmask(Interrupt::ADC_COMP);
        NVIC::unmask(Interrupt::DMA1_CHANNEL1);
        NVIC::unmask(Interrupt::EXTI4_15);
    }
    let exti = unsafe { &*stm32::EXTI::ptr() };
    exti.imr1()
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 15)) });

    let hal = crate::isr::TargetIsrHal {
        pwm,
        input,
        comp,
        interval,
        com_timer,
        phase,
    };
    InitResult {
        hal,
        sys,
        adc,
        telem,
    }
}
