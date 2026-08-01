//! Same as `bringup.rs`, but using typed PAC field accessors instead of raw
//! `.bits(0xN)` writes. Validates that the firmware-style accessor path
//! produces byte-identical register values to the hand-encoded bringup —
//! confirms no PAC-encoding gotchas (wrong bit shifts, write-1-to-clear
//! confusion, missing fields, etc.) before porting into rm32_stm32 proper.
//!
//! Build:
//!   cargo build --release --example bringup_pac --target thumbv7em-none-eabihf \
//!     --no-default-features --features stm32l431

#![no_std]
#![no_main]

use core::panic::PanicInfo;

use cortex_m_rt::entry;
use stm32l4xx_hal::pac;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    loop {
        cortex_m::asm::nop();
    }
}

#[entry]
fn main() -> ! {
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

    // --- PLL on, wait lock ---
    dp.RCC.cr.modify(|_, w| w.pllon().set_bit());
    while dp.RCC.cr.read().pllrdy().bit_is_clear() {}

    // --- SYSCLK = PLL (SW=0b11) ---
    dp.RCC.cfgr.modify(|_, w| unsafe { w.sw().bits(0b11) });
    while dp.RCC.cfgr.read().sws().bits() != 0b11 {}

    // --- NVIC PRIGROUP=3 (VECTKEY=0x05FA) ---
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

    // --- GPIOA pin config ---
    // Per-pin typed writes — equivalent to the 0xABD57FEF raw value:
    //   PA0/1/3/4/5/6/11/12 = analog (reset default), PA2 = AF, PA7 = output,
    //   PA8/9/10 = output (motor PWM idle safety), PA13/14/15 = AF (SWD).
    dp.GPIOA.moder.write(|w| unsafe {
        w.moder0()
            .bits(0b11)
            .moder1()
            .bits(0b11)
            .moder2()
            .bits(0b10) // PA2 AF (TIM15_CH1)
            .moder3()
            .bits(0b11)
            .moder4()
            .bits(0b11)
            .moder5()
            .bits(0b11)
            .moder6()
            .bits(0b11)
            .moder7()
            .bits(0b01) // PA7 output (TIM1_CH1N idle)
            .moder8()
            .bits(0b01) // PA8 output (TIM1_CH1 idle)
            .moder9()
            .bits(0b01) // PA9 output (TIM1_CH2 idle)
            .moder10()
            .bits(0b01) // PA10 output (TIM1_CH3 idle)
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
            .bits(0b01) // PA2 pull-up
            .pupdr13()
            .bits(0b01) // PA13 pull-up (AM32 overrides SWDIO reset's pull-down)
            .pupdr14()
            .bits(0b10) // PA14 pull-down (SWCLK reset default)
            .pupdr15()
            .bits(0b01) // PA15 pull-up (SWDIO/JTDI)
    });
    dp.GPIOA
        .afrl
        .write(|w| unsafe { w.afrl2().bits(14).afrl7().bits(1) });
    dp.GPIOA.afrh.write(|w| unsafe {
        w.afrh8()
            .bits(1) // TIM1_CH1
            .afrh9()
            .bits(1) // TIM1_CH2
            .afrh10()
            .bits(1) // TIM1_CH3
    });

    // --- GPIOB pin config (PB0/PB1 motor PWM N, PB6 USART1 half-duplex) ---
    // MODER target 0xFFFFEFB5 = PB0=AF (0b10@bits0-1=01? wait)
    //   0xFFFFEFB5 decode pin-by-pin:
    //     PB0 bits 0-1 = 0b01 → output (TIM1_CH2N idle, like PA7)
    //     PB1 bits 2-3 = 0b01 → output
    //     PB2 bits 4-5 = 0b11 → analog
    //     PB3 bits 6-7 = 0b11 → analog
    //     PB4 bits 8-9 = 0b11 → analog (BEMF input)
    //     PB5 bits 10-11 = 0b11 → analog
    //     PB6 bits 12-13 = 0b10 → AF (USART1)
    //     PB7 bits 14-15 = 0b11 → analog (BEMF)
    //     PB8-15 = analog (reset default)
    dp.GPIOB.moder.write(|w| unsafe {
        w.moder0()
            .bits(0b01)
            .moder1()
            .bits(0b01)
            .moder2()
            .bits(0b11)
            .moder3()
            .bits(0b10) // PB3 AF (SWO reset state — must preserve)
            .moder4()
            .bits(0b11)
            .moder5()
            .bits(0b11)
            .moder6()
            .bits(0b10) // PB6 AF USART1
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
    // AM32 clears PUPDR4 (reset has 0b01 pull-up on NJTRST). Explicit 0b00.
    dp.GPIOB
        .pupdr
        .write(|w| unsafe { w.pupdr4().bits(0b00).pupdr6().bits(0b01) });
    dp.GPIOB.afrl.write(|w| unsafe {
        w.afrl0()
            .bits(1) // TIM1_CH2N
            .afrl1()
            .bits(1) // TIM1_CH3N
            .afrl6()
            .bits(7) // USART1_TX
    });

    // --- TIM15 input capture ---
    dp.TIM15.psc.write(|w| unsafe { w.psc().bits(1) });
    dp.TIM15.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    // CCMR1 input mode: CC1S=01 (TI1), IC1F=0b0100 (fDTS/2, N=6)
    dp.TIM15
        .ccmr1_input()
        .write(|w| unsafe { w.cc1s().bits(0b01).ic1f().bits(0b0100) });
    // CCER: CC1E + CC1P + CC1NP (both-edge capture)
    dp.TIM15
        .ccer
        .write(|w| w.cc1e().set_bit().cc1p().set_bit().cc1np().set_bit());
    dp.TIM15.dier.write(|w| w.cc1de().set_bit());

    // --- DMA1 CH5 (TIM15_CH1 capture → ring buffer) ---
    dp.DMA1.cselr.modify(|_, w| unsafe {
        w.c4s()
            .bits(2) // USART1_TX request on CH4
            .c5s()
            .bits(7) // TIM15_CH1 request on CH5
    });
    dp.DMA1
        .cpar5
        .write(|w| unsafe { w.pa().bits(&dp.TIM15.ccr1 as *const _ as u32) });
    let buf_addr = core::ptr::addr_of!(DSHOT_BUF) as u32;
    dp.DMA1.cmar5.write(|w| unsafe { w.ma().bits(buf_addr) });
    dp.DMA1.cndtr5.write(|w| unsafe { w.ndt().bits(32) });
    // CCR5: EN + TCIE + TEIE + MINC + PSIZE=16 + MSIZE=32
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

    // --- TIM15 enable ---
    dp.TIM15.cr1.write(|w| w.cen().set_bit());

    // --- TIM1 (motor PWM) ---
    // CCMR1/2 = 0x6868: OC1M=0b0110 PWM1 + OC1PE preload; same for ch2.
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
    // CCER = 0x1555: CCxE + CCxNE for ch1/2/3, CC4E only.
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
    // BDTR = 0xA02D: MOE + BKP (break polarity high) + DTG=0x2D
    dp.TIM1
        .bdtr
        .write(|w| unsafe { w.moe().set_bit().bkp().set_bit().dtg().bits(0x2D) });
    dp.TIM1.cr1.write(|w| w.arpe().set_bit().cen().set_bit());

    // --- TIM2 (free-running 2 MHz μs counter) ---
    dp.TIM2.psc.write(|w| unsafe { w.psc().bits(0x27) });
    dp.TIM2.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    dp.TIM2.cr1.write(|w| w.cen().set_bit());

    // --- TIM6 (system tick ~20 kHz) ---
    dp.TIM6.psc.write(|w| unsafe { w.psc().bits(0x4F) });
    dp.TIM6.arr.write(|w| unsafe { w.arr().bits(0x32) });
    dp.TIM6.dier.write(|w| w.uie().set_bit());
    dp.TIM6.cr1.write(|w| w.cen().set_bit());

    // --- TIM7 (1 MHz μs counter) ---
    dp.TIM7.psc.write(|w| unsafe { w.psc().bits(0x4F) });
    dp.TIM7.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    dp.TIM7.cr1.write(|w| w.cen().set_bit());

    // --- TIM16 (commutation timer @ 2 MHz tick) ---
    dp.TIM16.psc.write(|w| unsafe { w.psc().bits(0x27) });
    dp.TIM16.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    dp.TIM16.cr1.write(|w| w.arpe().set_bit().cen().set_bit());

    // --- DMA1 CH4 (USART1_TX path; channel idle) ---
    // CCR4 = 0x9A: TCIE + TEIE + DIR(mem→peri) + MINC. PSIZE/MSIZE both 8-bit.
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

    // --- USART1 (half-duplex 115200 8N1, DMA TX) ---
    dp.USART1.brr.write(|w| unsafe { w.bits(0x2B6) });
    dp.USART1
        .cr3
        .write(|w| w.hdsel().set_bit().dmat().set_bit());
    dp.USART1.cr1.write(|w| w.ue().set_bit().re().set_bit());

    // --- IWDG (PR=2 /16, RLR=0xFA0 → ~2 s timeout) — MUST start to commit ---
    dp.IWDG.kr.write(|w| unsafe { w.key().bits(0x5555) });
    dp.IWDG.pr.write(|w| unsafe { w.pr().bits(2) });
    dp.IWDG.rlr.write(|w| unsafe { w.rl().bits(0xFA0) });
    dp.IWDG.kr.write(|w| unsafe { w.key().bits(0xCCCC) });
    while dp.IWDG.sr.read().bits() != 0 {}

    // --- COMP2 (BEMF) — two-stage to avoid LOCK firing before config commits ---
    let comp = unsafe { &*pac::COMP::PTR };
    comp.comp2_csr.write(|w| unsafe { w.bits(0x0000_0071) });
    comp.comp2_csr.write(|w| unsafe { w.bits(0x4000_0071) });

    loop {
        dp.IWDG.kr.write(|w| unsafe { w.key().bits(0xAAAA) });
        for _ in 0..1000 {
            cortex_m::asm::nop();
        }
    }
}

#[unsafe(link_section = ".bss")]
static mut DSHOT_BUF: [u32; 32] = [0; 32];

#[unsafe(link_section = ".bss")]
static mut USART_TX_BUF: [u8; 64] = [0; 64];
