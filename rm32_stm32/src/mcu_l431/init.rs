//! STM32L431 initialization.

use crate::init::InitResult;
use crate::mcu::ChipConfig;
use crate::phase::G0APhaseDriver;
use crate::timer::{Tim2Interval, Tim14Com};

pub fn init(
    dead_time: u8,
    bemf_pins: rm32::board::BemfPins,
) -> InitResult<super::system::System, super::adc::L431Adc, super::telemetry_uart::L431TelemUart> {
    use stm32l4xx_hal::pac;

    let dp = pac::Peripherals::take().unwrap();
    let _cp = cortex_m::Peripherals::take().unwrap();

    // Clock tree: AM32-matched. 80 MHz SYSCLK = HSI16 / 2 * 20 / 2 (M=2/N=20/R=2).
    // The stm32l4xx-hal `cfgr.sysclk(80MHz).freeze()` picks M=1/N=10/R=2 instead
    // — same final frequency but different VCO and PLL jitter spectrum, plus a
    // different RCC.PLLCFGR readback. We need byte-for-byte parity with AM32,
    // so do the register writes directly.
    //
    // Order is load-bearing: FLASH wait states MUST be raised to 4 before
    // SYSCLK exceeds 16 MHz, or the CPU may fetch garbage instructions.
    //
    // PRFTEN: deliberately ON — a documented divergence from AM32 (which
    // runs prefetch off). The minz saga (board_init.rs, commit 8d32e66):
    // PRFTEN off + 4WS makes hot-ISR fetch timing depend on code
    // alignment, so ANY rebuild moves marginal timing (engage went
    // 3-4/8 -> 8/8 from this one bit, same binary otherwise). The clone —
    // our behavioral parity reference — runs prefetch on. It is also the
    // prime suspect for rm32's polarity-correlated ZC accept excursions
    // (the rising/falling persistence branches sit at different flash
    // alignments; without prefetch the two polarities sample at different
    // effective cadence).
    dp.FLASH.acr.write(|w| unsafe {
        w.latency()
            .bits(4)
            .prften()
            .set_bit()
            .icen()
            .set_bit()
            .dcen()
            .set_bit()
    });
    while dp.FLASH.acr.read().latency().bits() != 4 {}

    dp.RCC.cr.modify(|_, w| w.hsion().set_bit());
    while dp.RCC.cr.read().hsirdy().bit_is_clear() {}

    dp.RCC.pllcfgr.write(|w| unsafe {
        w.pllsrc()
            .bits(2) // HSI16
            .pllm()
            .bits(1) // /2
            .plln()
            .bits(20)
            .pllr()
            .bits(0) // /2
            .pllren()
            .set_bit()
    });

    dp.RCC.cr.modify(|_, w| w.pllon().set_bit());
    while dp.RCC.cr.read().pllrdy().bit_is_clear() {}

    dp.RCC.cfgr.modify(|_, w| unsafe { w.sw().bits(0b11) });
    while dp.RCC.cfgr.read().sws().bits() != 0b11 {}

    // NVIC priority grouping = 3 (4 bits preempt / 0 subpriority on Cortex-M4).
    // AM32 does this via HAL_NVIC_SetPriorityGrouping. Affects how priorities
    // assigned to peripheral IRQs get interpreted by the NVIC; matching here
    // keeps ISR preemption behavior identical to AM32.
    unsafe {
        core::ptr::write_volatile(0xE000_ED0C as *mut u32, 0x05FA_0300);
    }

    let rcc_pac = unsafe { &*pac::RCC::ptr() };
    unsafe {
        // Enable GPIOA, GPIOB (AHB2ENR bits 0, 1)
        rcc_pac
            .ahb2enr
            .modify(|_, w| w.gpioaen().set_bit().gpioben().set_bit());
        // Enable TIM1 (APB2ENR bit 11)
        rcc_pac.apb2enr.modify(|_, w| w.tim1en().set_bit());

        // PA8/9/10 as AF1 (TIM1_CH1/2/3 on L4 = AF1, not AF2)
        let gpioa = &*pac::GPIOA::ptr();
        gpioa.moder.modify(|r, w| {
            w.bits(
                (r.bits() & !(0b11 << 16 | 0b11 << 18 | 0b11 << 20))
                    | (0b10 << 16 | 0b10 << 18 | 0b10 << 20),
            )
        });
        gpioa.afrh.modify(|r, w| {
            w.bits((r.bits() & !(0xFFF)) | (1 | 1 << 4 | 1 << 8)) // AF1
        });
        // PA7 = TIM1_CH1N (AF1). AM32 sets the AFRL bits even though MODER
        // stays as OUTPUT (safety idle — PA7 stays driven low by GPIO until
        // motor is armed). Matching the AFRL bits is parity-only here.
        gpioa.afrl.modify(|_, w| w.afrl7().bits(1));
        // PB0 = TIM1_CH2N, PB1 = TIM1_CH3N (both AF1). Same safety pattern.
        // PB6 = USART1_TX (AF7) half-duplex telemetry pad — set MODER=AF +
        //       PUPDR=pull-up + AFRL=7 here so the line idles high regardless
        //       of which USART1 path (`debuguart` or `telemetry_uart`) takes
        //       over later (both use .modify() and preserve these bits).
        // PB4  = reset state has PUPDR=0b01 (NJTRST pull-up). AM32 clears it.
        let gpiob = &*pac::GPIOB::ptr();
        gpiob
            .afrl
            .modify(|_, w| w.afrl0().bits(1).afrl1().bits(1).afrl6().bits(7));
        gpiob.moder.modify(|_, w| w.moder6().bits(0b10));
        gpiob
            .pupdr
            .modify(|_, w| w.pupdr4().bits(0b00).pupdr6().bits(0b01));
    }

    // TIM1 PWM: 80 MHz / (ARR+1) = 24 kHz, motor PWM. AM32-matched fields.
    //
    // - CCMR1 = 0x6868: OC1/2 PWM mode 1 (0b110) with output preload (OCxPE=1).
    // - CCMR2 = 0x6868: same for OC3/4 (AM32 has both ch3 AND ch4 PWM-mode'd
    //   with preload, even though ch4 isn't routed to a pin — used internally
    //   as TRGO source for sample-and-hold timing).
    // - CCER  = 0x1555: CC1E + CC1NE + CC2E + CC2NE + CC3E + CC3NE + CC4E.
    //   Old code wrote 0x555 (no CH4E), which prevented the CCR4 trigger.
    // - CCR4  = 0x64 (100): trigger value for OC4REF (TRGO_4, used for BEMF
    //   sample timing).
    // - BDTR  = DTG | BKP | MOE. Old code missed BKP (break input active-high
    //   polarity) — without it, BRK behaves opposite to AM32 if a break event
    //   ever fires.
    // - CR1   = ARPE + CEN. Old code missed ARPE (auto-reload preload),
    //   meaning ARR writes take effect immediately instead of on next update.
    unsafe {
        let tim1 = &*pac::TIM1::ptr();
        tim1.psc.write(|w| w.psc().bits(0));
        tim1.arr
            .write(|w| w.arr().bits(super::chip::Chip::TIM1_AUTORELOAD));
        tim1.ccmr1_output().write(|w| {
            w.oc1m()
                .bits(0b110)
                .oc1pe()
                .set_bit()
                .oc2m()
                .bits(0b110)
                .oc2pe()
                .set_bit()
        });
        tim1.ccmr2_output().write(|w| {
            w.oc3m()
                .bits(0b110)
                .oc3pe()
                .set_bit()
                .oc4m()
                .bits(0b110)
                .oc4pe()
                .set_bit()
        });
        tim1.ccer.write(|w| {
            w.cc1e()
                .set_bit()
                .cc1ne()
                .set_bit()
                .cc2e()
                .set_bit()
                .cc2ne()
                .set_bit()
                .cc3e()
                .set_bit()
                .cc3ne()
                .set_bit()
                .cc4e()
                .set_bit()
        });
        tim1.ccr4.write(|w| w.ccr().bits(0x64));
        tim1.bdtr
            .write(|w| w.moe().set_bit().bkp().set_bit().dtg().bits(dead_time));
        tim1.cr1.write(|w| w.arpe().set_bit().cen().set_bit());
    }
    let pwm = super::pwm::Pwm::new();
    let phase = G0APhaseDriver::new(false); // same pins for L4_N

    // COMP2 init — use phase_a from board config for initial INMSEL
    super::comp_init::init_comp2(bemf_pins.phase_a);
    let comp = super::comparator::new_comparator(bemf_pins);
    let interval = Tim2Interval::new();
    let com_timer = Tim14Com::new(); // L431 uses TIM16, but TIM14 struct works (same register layout)

    // Input capture (TIM15 + DMA1_CH5)
    super::input_capture::init_l431();
    let mut input = super::input_capture::new_capture();
    use rm32::hal::InputCapture;
    input.receive_dshot_dma();

    // ADC
    let adc = super::adc::new_adc();
    let _ = adc.init();

    // UART telemetry. `debuguart` feature steals USART1+PB6 for debug logging,
    // so skip the telemetry hardware init in that build — leaves the wrapper
    // in place (so main.rs still compiles + can call send_dma, which becomes
    // a harmless no-op on uninitialized DMA) without stomping on debug_uart.
    #[cfg(feature = "debuguart")]
    let telem = super::telemetry_uart::L431TelemUart::post_init();
    #[cfg(not(feature = "debuguart"))]
    let telem = super::telemetry_uart::L431TelemUart::init()
        .unwrap_or_else(|_| super::telemetry_uart::L431TelemUart::post_init());

    // TIM6: AM32-matched decomposition. 80 MHz / (PSC+1) / (ARR+1) = 80M/80/51
    // ≈ 19.6 kHz. Previous code used PSC=0/ARR=3999 (same 20 kHz mean rate but
    // single 80 MHz tick — counter rolls every 50 μs instead of every 51 μs at
    // 1 MHz). AM32 ticks at 1 MHz internally; the synchronizer between APB1
    // and the timer block sees different jitter, and any code that samples
    // TIM6.CNT mid-period would read different values. Match AM32 exactly.
    //
    // Also enable TIM7 clock (AM32 enables it even though firmware doesn't use
    // it yet) so RCC.APB1ENR1 matches byte-for-byte.
    unsafe {
        rcc_pac
            .apb1enr1
            .modify(|_, w| w.tim6en().set_bit().tim7en().set_bit());
        let tim6 = &*pac::TIM6::ptr();
        tim6.psc.write(|w| w.psc().bits(0x4F)); // 79 → 1 MHz tick
        tim6.arr.write(|w| w.arr().bits(0x32)); // 50 → 51-tick period
        tim6.egr.write(|w| w.ug().set_bit());
        tim6.sr.write(|w| w.uif().clear_bit());
        tim6.dier.write(|w| w.uie().set_bit());
        tim6.cr1.write(|w| w.cen().set_bit());

        // TIM7: AM32 runs it as a free-running 1 MHz μs counter. Firmware
        // doesn't currently consume it, but match AM32 for register parity.
        let tim7 = &*pac::TIM7::ptr();
        tim7.psc.write(|w| w.psc().bits(0x4F));
        tim7.arr.write(|w| w.arr().bits(0xFFFF));
        tim7.cr1.write(|w| w.cen().set_bit());
    }

    // NVIC priorities — AM32-matched. On STM32L4 the NVIC has 4 priority bits
    // stored in the UPPER nibble of each IPR byte (PRIGROUP=3 from clock init
    // = 4 preempt bits, 0 sub bits). cortex_m's `set_priority` writes the raw
    // byte, so to use priority level N we must pass `N << 4` (AM32 uses the
    // CMSIS NVIC_SetPriority macro which does this shift internally).
    //
    // Priorities (lower number = higher priority, can preempt higher numbers):
    //   COMP             0  — BEMF zero-cross, must preempt tenKhzRoutine
    //   TIM1_UP_TIM16    0  — commutation timer, same urgency as COMP
    //   DMA1_CH5         1  — DSHOT/PWM input capture
    //   EXTI15_10        2  — SW-triggered frame processing
    //   TIM6_DACUNDER    3  — 20 kHz control loop (LOWEST: tenKhzRoutine can
    //                          run long and be preempted by motor-critical IRQs)
    //
    // Without these priorities, all IRQs default to 0 and TIM6 ISR blocks
    // commutation/BEMF until it returns. tenKhzRoutine can take ~45 µs of the
    // 50 µs TIM6 budget; without preemption, commutation timing decays under
    // load → motor chops.
    unsafe {
        use pac::{Interrupt, NVIC};
        NVIC::unmask(Interrupt::TIM6_DACUNDER);
        NVIC::unmask(Interrupt::TIM1_UP_TIM16);
        NVIC::unmask(Interrupt::COMP);
        NVIC::unmask(Interrupt::DMA1_CH5);
        NVIC::unmask(Interrupt::EXTI15_10);
        let mut nvic = cortex_m::Peripherals::steal().NVIC;
        nvic.set_priority(Interrupt::COMP, 0 << 4);
        nvic.set_priority(Interrupt::TIM1_UP_TIM16, 0 << 4);
        nvic.set_priority(Interrupt::DMA1_CH5, 1 << 4);
        nvic.set_priority(Interrupt::EXTI15_10, 2 << 4);
        nvic.set_priority(Interrupt::TIM6_DACUNDER, 3 << 4);
        // benchuart: USART2 RX at level 2 (matches the clone). The vector
        // is ring-push-only — never touches ISR_LOCAL — so the IsrCell
        // aliasing rule does not constrain its priority.
        #[cfg(feature = "benchuart")]
        {
            NVIC::unmask(Interrupt::USART2);
            nvic.set_priority(Interrupt::USART2, 2 << 4);
        }
    }

    // Enable EXTI line 15 (software-triggered by DMA TC)
    unsafe {
        let exti = &*pac::EXTI::ptr();
        exti.imr1.modify(|r, w| w.bits(r.bits() | (1 << 15)));
    }

    let sys = super::system::System::new();
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
