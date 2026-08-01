//! Temporary bring-up path — register-level AM32 parity, no firmware logic.
//!
//! Gated behind `feature = "bringup"`. When on, `main()` jumps straight into
//! `run_and_spin()` instead of running normal init. The purpose is to verify
//! that the firmware (rm32_firmware) reaches the same peripheral register
//! state AM32 does, before we start replacing pieces of the real init path.
//!
//! Source of truth for the values lives in `examples/bringup_pac.rs` — this
//! module is the same code lifted into the firmware crate so it can be flashed
//! via the normal `cargo run` runner.
//!
//! Diff workflow:
//!   1. Flash AM32: `probe-rs download --binary-format hex AM32_VIMDRONES_L431_2.20.hex`
//!      then `python scripts/dump_l431_regs.py > am32_regs.txt`
//!   2. Build + flash this:
//!      `cargo run --release --no-default-features --features stm32l431,bringup`
//!   3. `python scripts/dump_l431_regs.py > rm32_regs.txt`
//!   4. `diff am32_regs.txt rm32_regs.txt` — should differ only on runtime state
//!      (counter values, capture flags, DMA already-completed flags, etc.).

use crate::pac;

#[unsafe(link_section = ".bss")]
static mut DSHOT_BUF: [u32; 32] = [0; 32];

#[unsafe(link_section = ".bss")]
static mut USART_TX_BUF: [u8; 64] = [0; 64];

/// Run the AM32-matching register init and spin forever.
///
/// Called from `main()` when `feature = "bringup"` is on. Refreshes IWDG in
/// the spin loop so the chip stays alive long enough for register dumps.
pub fn run_and_spin() -> ! {
    let dp = unsafe { pac::Peripherals::steal() };

    // --- FLASH wait states (LATENCY=4, ICEN+DCEN; AM32 leaves PRFTEN=0) ---
    dp.FLASH
        .acr
        .write(|w| unsafe { w.latency().bits(4).icen().set_bit().dcen().set_bit() });
    while dp.FLASH.acr.read().latency().bits() != 4 {}

    // --- HSI16 on ---
    dp.RCC.cr.modify(|_, w| w.hsion().set_bit());
    while dp.RCC.cr.read().hsirdy().bit_is_clear() {}

    // --- PLL: PLLSRC=HSI16, PLLM=1(→/2), PLLN=20, PLLR=0(→/2), PLLREN ---
    dp.RCC.pllcfgr.write(|w| unsafe {
        w.pllsrc()
            .bits(2)
            .pllm()
            .bits(1)
            .plln()
            .bits(20)
            .pllr()
            .bits(0)
            .pllren()
            .set_bit()
    });

    dp.RCC.cr.modify(|_, w| w.pllon().set_bit());
    while dp.RCC.cr.read().pllrdy().bit_is_clear() {}

    dp.RCC.cfgr.modify(|_, w| unsafe { w.sw().bits(0b11) });
    while dp.RCC.cfgr.read().sws().bits() != 0b11 {}

    // --- NVIC PRIGROUP=3 ---
    unsafe {
        core::ptr::write_volatile(0xE000_ED0C as *mut u32, 0x05FA_0300);
    }

    // --- RCC peripheral clock enables ---
    dp.RCC.ahb1enr.modify(|_, w| w.dma1en().set_bit());
    dp.RCC.ahb2enr.modify(|_, w| w.adcen().set_bit());
    dp.RCC
        .apb1enr1
        .modify(|_, w| w.tim6en().set_bit().tim7en().set_bit());
    dp.RCC.apb2enr.modify(|_, w| {
        w.syscfgen()
            .set_bit()
            .tim1en()
            .set_bit()
            .usart1en()
            .set_bit()
            .tim15en()
            .set_bit()
            .tim16en()
            .set_bit()
    });
    let _ = dp.RCC.apb2enr.read().bits();

    // --- GPIOA: PA2 AF (TIM15_CH1), PA7-10 output (motor PWM idle safety) ---
    dp.GPIOA.moder.write(|w| unsafe {
        w.moder0()
            .bits(0b11)
            .moder1()
            .bits(0b11)
            .moder2()
            .bits(0b10)
            .moder3()
            .bits(0b11)
            .moder4()
            .bits(0b11)
            .moder5()
            .bits(0b11)
            .moder6()
            .bits(0b11)
            .moder7()
            .bits(0b01)
            .moder8()
            .bits(0b01)
            .moder9()
            .bits(0b01)
            .moder10()
            .bits(0b01)
            .moder11()
            .bits(0b11)
            .moder12()
            .bits(0b11)
            .moder13()
            .bits(0b10)
            .moder14()
            .bits(0b10)
            .moder15()
            .bits(0b10)
    });
    dp.GPIOA
        .ospeedr
        .write(|w| unsafe { w.ospeedr2().bits(0b10).ospeedr13().bits(0b11) });
    dp.GPIOA.pupdr.write(|w| unsafe {
        w.pupdr2()
            .bits(0b01)
            .pupdr13()
            .bits(0b01)
            .pupdr14()
            .bits(0b10)
            .pupdr15()
            .bits(0b01)
    });
    dp.GPIOA
        .afrl
        .write(|w| unsafe { w.afrl2().bits(14).afrl7().bits(1) });
    dp.GPIOA
        .afrh
        .write(|w| unsafe { w.afrh8().bits(1).afrh9().bits(1).afrh10().bits(1) });

    // --- GPIOB: PB0/PB1 motor PWM N (output idle), PB3 SWO AF, PB6 USART1 AF ---
    dp.GPIOB.moder.write(|w| unsafe {
        w.moder0()
            .bits(0b01)
            .moder1()
            .bits(0b01)
            .moder2()
            .bits(0b11)
            .moder3()
            .bits(0b10)
            .moder4()
            .bits(0b11)
            .moder5()
            .bits(0b11)
            .moder6()
            .bits(0b10)
            .moder7()
            .bits(0b11)
            .moder8()
            .bits(0b11)
            .moder9()
            .bits(0b11)
            .moder10()
            .bits(0b11)
            .moder11()
            .bits(0b11)
            .moder12()
            .bits(0b11)
            .moder13()
            .bits(0b11)
            .moder14()
            .bits(0b11)
            .moder15()
            .bits(0b11)
    });
    dp.GPIOB
        .pupdr
        .write(|w| unsafe { w.pupdr4().bits(0b00).pupdr6().bits(0b01) });
    dp.GPIOB
        .afrl
        .write(|w| unsafe { w.afrl0().bits(1).afrl1().bits(1).afrl6().bits(7) });

    // --- TIM15 input capture (DSHOT bit timing) ---
    dp.TIM15.psc.write(|w| unsafe { w.psc().bits(1) });
    dp.TIM15.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    dp.TIM15
        .ccmr1_input()
        .write(|w| unsafe { w.cc1s().bits(0b01).ic1f().bits(0b0100) });
    dp.TIM15
        .ccer
        .write(|w| w.cc1e().set_bit().cc1p().set_bit().cc1np().set_bit());
    dp.TIM15.dier.write(|w| w.cc1de().set_bit());

    // --- DMA1 CH5 (TIM15_CH1 → DSHOT_BUF) ---
    dp.DMA1
        .cselr
        .modify(|_, w| unsafe { w.c4s().bits(2).c5s().bits(7) });
    dp.DMA1
        .cpar5
        .write(|w| unsafe { w.pa().bits(&dp.TIM15.ccr1 as *const _ as u32) });
    dp.DMA1
        .cmar5
        .write(|w| unsafe { w.ma().bits(core::ptr::addr_of!(DSHOT_BUF) as u32) });
    dp.DMA1.cndtr5.write(|w| unsafe { w.ndt().bits(32) });
    dp.DMA1.ccr5.write(|w| unsafe {
        w.en()
            .set_bit()
            .tcie()
            .set_bit()
            .teie()
            .set_bit()
            .minc()
            .set_bit()
            .psize()
            .bits(0b01)
            .msize()
            .bits(0b10)
    });
    dp.TIM15.cr1.write(|w| w.cen().set_bit());

    // --- TIM1 motor PWM (80 MHz / 3333 ≈ 24 kHz, dead-time 45 ticks ≈ 562 ns) ---
    dp.TIM1.psc.write(|w| unsafe { w.psc().bits(0) });
    dp.TIM1.arr.write(|w| unsafe { w.arr().bits(0x0D04) });
    dp.TIM1.ccmr1_output().write(|w| unsafe {
        w.oc1m()
            .bits(0b110)
            .oc1pe()
            .set_bit()
            .oc2m()
            .bits(0b110)
            .oc2pe()
            .set_bit()
    });
    dp.TIM1.ccmr2_output().write(|w| unsafe {
        w.oc3m()
            .bits(0b110)
            .oc3pe()
            .set_bit()
            .oc4m()
            .bits(0b110)
            .oc4pe()
            .set_bit()
    });
    dp.TIM1.ccer.write(|w| {
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
    dp.TIM1.ccr4.write(|w| unsafe { w.ccr().bits(0x64) });
    dp.TIM1
        .bdtr
        .write(|w| unsafe { w.moe().set_bit().bkp().set_bit().dtg().bits(0x2D) });
    dp.TIM1.cr1.write(|w| w.arpe().set_bit().cen().set_bit());

    // --- TIM2 (2 MHz μs counter), TIM6 (20 kHz tick), TIM7 (1 MHz μs counter),
    //     TIM16 (commutation 2 MHz) ---
    dp.TIM2.psc.write(|w| unsafe { w.psc().bits(0x27) });
    dp.TIM2.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    dp.TIM2.cr1.write(|w| w.cen().set_bit());

    dp.TIM6.psc.write(|w| unsafe { w.psc().bits(0x4F) });
    dp.TIM6.arr.write(|w| unsafe { w.arr().bits(0x32) });
    dp.TIM6.dier.write(|w| w.uie().set_bit());
    dp.TIM6.cr1.write(|w| w.cen().set_bit());

    dp.TIM7.psc.write(|w| unsafe { w.psc().bits(0x4F) });
    dp.TIM7.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    dp.TIM7.cr1.write(|w| w.cen().set_bit());

    dp.TIM16.psc.write(|w| unsafe { w.psc().bits(0x27) });
    dp.TIM16.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    dp.TIM16.cr1.write(|w| w.arpe().set_bit().cen().set_bit());

    // --- DMA1 CH4 (USART1_TX, idle until needed) ---
    dp.DMA1.ccr4.write(|w| {
        w.tcie()
            .set_bit()
            .teie()
            .set_bit()
            .dir()
            .set_bit()
            .minc()
            .set_bit()
    });
    dp.DMA1
        .cpar4
        .write(|w| unsafe { w.pa().bits(&dp.USART1.tdr as *const _ as u32) });
    dp.DMA1
        .cmar4
        .write(|w| unsafe { w.ma().bits(core::ptr::addr_of!(USART_TX_BUF) as u32) });

    // --- USART1 half-duplex 115200 8N1, DMA TX ---
    dp.USART1.brr.write(|w| unsafe { w.bits(0x2B6) });
    dp.USART1
        .cr3
        .write(|w| w.hdsel().set_bit().dmat().set_bit());
    dp.USART1.cr1.write(|w| w.ue().set_bit().re().set_bit());

    // --- IWDG start (PR=2 /16, RLR=0xFA0 → ~2 s timeout) ---
    dp.IWDG.kr.write(|w| unsafe { w.key().bits(0x5555) });
    dp.IWDG.pr.write(|w| unsafe { w.pr().bits(2) });
    dp.IWDG.rlr.write(|w| unsafe { w.rl().bits(0xFA0) });
    dp.IWDG.kr.write(|w| unsafe { w.key().bits(0xCCCC) });
    while dp.IWDG.sr.read().bits() != 0 {}

    // --- COMP2 (BEMF) — two-stage to commit config before LOCK latches ---
    let comp = unsafe { &*pac::COMP::PTR };
    comp.comp2_csr.write(|w| unsafe { w.bits(0x0000_0071) });
    comp.comp2_csr.write(|w| unsafe { w.bits(0x4000_0071) });

    // Spin with periodic IWDG refresh so SWD AHB-AP reads stay live AND the
    // chip doesn't watchdog-reset before the user dumps registers.
    loop {
        dp.IWDG.kr.write(|w| unsafe { w.key().bits(0xAAAA) });
        for _ in 0..1000 {
            cortex_m::asm::nop();
        }
    }
}
