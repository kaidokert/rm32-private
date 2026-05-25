//! Minimal L431 clock-tree bring-up — bit-for-bit parity check vs AM32.
//!
//! Goal: produce the same RCC.CR / RCC.CFGR / RCC.PLLCFGR / FLASH.ACR values
//! that `am32_regs.txt` shows. Nothing else. No peripherals, no GPIO, no
//! interrupts. Just sysclk at 80 MHz via PLL, then `wfi`.
//!
//! AM32's target values (from `am32_regs.txt`):
//!   FLASH.ACR    = 0x00000604   (LATENCY=4, PRFTEN, ICEN, DCEN)
//!   RCC.PLLCFGR  = 0x01001412   (PLLSRC=HSI16, PLLM=2, PLLN=20, PLLR=2, PLLREN)
//!   RCC.CFGR     = 0x0000000F   (SW=PLL, SWS=PLL)
//!   RCC.CR       = PLLON+PLLRDY+HSION+HSIRDY (MSI bits left at reset default)
//!
//! Validate by flashing this binary, then:
//!   python scripts/dump_l431_regs.py > bringup_regs.txt
//!   diff am32_regs.txt bringup_regs.txt
//!
//! Build:
//!   cargo build --release --example bringup --target thumbv7em-none-eabihf \
//!     --no-default-features --features stm32l431
//! Flash:
//!   cargo run   --release --example bringup --target thumbv7em-none-eabihf \
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
    // SAFETY: single-threaded, pre-init, exclusive access to all peripherals.
    let dp = unsafe { pac::Peripherals::steal() };

    // --- FLASH wait states FIRST (must be >=4 before going to 80 MHz) ---
    // 0x00000604 = LATENCY=4 | ICEN(bit 9) | DCEN(bit 10). PRFTEN (bit 8) is
    // OFF — AM32 deliberately disables the prefetch unit (caches handle it).
    // Write the full word — no read-modify-write — to guarantee parity.
    dp.FLASH.acr.write(|w| unsafe { w.bits(0x0000_0604) });
    // Re-read to verify latency took (datasheet recommends).
    while dp.FLASH.acr.read().bits() & 0x7 != 4 {}

    // --- HSI16 on (PLL source) ---
    dp.RCC.cr.modify(|_, w| w.hsion().set_bit());
    while dp.RCC.cr.read().hsirdy().bit_is_clear() {}

    // --- PLL configuration (PLL must be OFF — it is at reset) ---
    // 0x01001412 = PLLREN | (PLLN=20)<<8 | (PLLM=1 → /2)<<4 | (PLLSRC=2, HSI16).
    // PLLR encoding 0b00 → /2. PLLP/PLLQ disabled.
    dp.RCC.pllcfgr.write(|w| unsafe { w.bits(0x0100_1412) });

    // --- PLL on, wait for lock ---
    dp.RCC.cr.modify(|_, w| w.pllon().set_bit());
    while dp.RCC.cr.read().pllrdy().bit_is_clear() {}

    // --- Switch SYSCLK to PLL (SW=0b11) ---
    dp.RCC.cfgr.modify(|_, w| unsafe { w.sw().bits(0b11) });
    // Confirm switchover: SWS bits 2..3 == 0b11.
    while (dp.RCC.cfgr.read().bits() >> 2) & 0b11 != 0b11 {}

    // SYSCLK is now 80 MHz from PLL.
    // AHB/APB1/APB2 prescalers remain at reset default (/1, /1, /1) which matches AM32.

    // --- NVIC priority grouping ---
    // AM32 calls HAL_NVIC_SetPriorityGrouping(NVIC_PRIORITYGROUP_3) which writes
    // SCB.AIRCR with VECTKEY=0x05FA + PRIGROUP=3 (4 bits preempt / 0 bits sub on
    // Cortex-M4). Read-back is 0xFA050300 (VECTKEYSTAT|PRIGROUP<<8).
    unsafe {
        core::ptr::write_volatile(0xE000_ED0C as *mut u32, 0x05FA_0300);
    }

    // --- RCC peripheral-clock enables (mirror AM32's __HAL_RCC_*_CLK_ENABLE) ---
    // AHB1ENR: DMA1EN (FLASHEN already on from boot).
    dp.RCC.ahb1enr.modify(|_, w| w.dma1en().set_bit());
    // AHB2ENR: ADCEN (GPIOA/GPIOB already on from boot).
    dp.RCC.ahb2enr.modify(|_, w| w.adcen().set_bit());
    // APB1ENR1: TIM6EN + TIM7EN (TIM2/WWDG/PWR already on from boot).
    dp.RCC
        .apb1enr1
        .modify(|_, w| w.tim6en().set_bit().tim7en().set_bit());
    // APB2ENR: SYSCFG + TIM1 + USART1 + TIM15 + TIM16. AM32 uses USART1 for
    // the KISS telemetry pad on PB6 (NOT SPI1 — APB2 bit 14 not bit 12).
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
    // Read-back fence (DMB-equivalent) — Cortex-M4 errata recommends a dummy
    // read after enabling a peripheral clock before touching its registers.
    let _ = dp.RCC.apb2enr.read().bits();

    // --- GPIOA pin config (DSHOT signal + motor PWM idle state) ---
    // PA2  = TIM15_CH1 input (AF14) for DSHOT signal capture, pull-up, medium speed.
    // PA7  = TIM1_CH1N AF1 selected in AFRL, but MODER kept as OUTPUT —
    //        AM32 leaves the motor low-side N pins as GPIO outputs until armed,
    //        then flips MODER to AF for PWM. Safety idle.
    // PA8/9/10 = TIM1_CH1/2/3 high-side: MODER kept as OUTPUT for same reason.
    //
    // Target MODER = 0xABD57FEF (matches am32_regs.txt).
    dp.GPIOA.moder.write(|w| unsafe { w.bits(0xABD5_7FEF) });
    // OSPEEDR: PA2 medium (0b10), PA13 high (0b11) preserved from SWD config.
    dp.GPIOA.ospeedr.write(|w| unsafe { w.bits(0x0C00_0020) });
    // PUPDR: PA2 pull-up (0b01 @ bits 4-5), PA13 pull-down (SWDIO), PA15 pull-up.
    dp.GPIOA.pupdr.write(|w| unsafe { w.bits(0x6400_0010) });
    // AFRL: PA2 = AF14 (TIM15_CH1) @ bits 8-11; PA7 = AF1 (TIM1_CH1N) @ bits 28-31.
    dp.GPIOA.afrl.write(|w| unsafe { w.bits(0x1000_0E00) });
    // AFRH: PA8/PA9/PA10 = AF1 (TIM1_CH1/2/3). MODER stays output until armed.
    dp.GPIOA.afrh.write(|w| unsafe { w.bits(0x0000_0111) });

    // --- GPIOB pin config (motor PWM low-side complementary + USART1 TX) ---
    // PB0 = TIM1_CH2N AF1, PB1 = TIM1_CH3N AF1, PB6 = USART1_TX/RX AF7 half-duplex.
    // Target MODER = 0xFFFFEFB5 (matches am32_regs.txt).
    dp.GPIOB.moder.write(|w| unsafe { w.bits(0xFFFF_EFB5) });
    // PUPDR PB6 = pull-up (bit 12 = 0b01) — USART idle high.
    dp.GPIOB.pupdr.write(|w| unsafe { w.bits(0x0000_1000) });
    // AFRL: PB0=AF1, PB1=AF1, PB6=AF7 → 0x07000011.
    dp.GPIOB.afrl.write(|w| unsafe { w.bits(0x0700_0011) });

    // --- TIM15 input capture init (DSHOT bit timing capture) ---
    // PSC=1 → 40 MHz tick (matches AM32 DSHOT600 boot default).
    dp.TIM15.psc.write(|w| unsafe { w.bits(1) });
    // ARR=0xFFFF reset default (16-bit free-run wrap). Write explicit for parity.
    dp.TIM15.arr.write(|w| unsafe { w.bits(0xFFFF) });
    // CCMR1 = 0x41: CC1S=01 (input on TI1), IC1F=0b0100 (fDTS/2, N=6 filter).
    dp.TIM15.ccmr1_input().write(|w| unsafe { w.bits(0x41) });
    // CCER = 0x0B: CC1E=1 + CC1P=1 + CC1NP=1 → both-edge capture on TI1.
    dp.TIM15.ccer.write(|w| unsafe { w.bits(0x0B) });
    // DIER = 0x200: CC1DE=1 (DMA request on capture event).
    dp.TIM15.dier.write(|w| unsafe { w.bits(0x200) });

    // --- DMA1 CH5 init (TIM15_CH1 → memory ring buffer) ---
    // CSELR.C5S = 7 (TIM15_CH1 request, per L431 RM table 41) +
    // CSELR.C4S = 2 (USART1_TX request — AM32 wires it here even before USART
    // init so the mux matches; channel itself stays disabled until needed).
    dp.DMA1
        .cselr
        .modify(|r, w| unsafe { w.bits((r.bits() & !0xFF_F000) | (2 << 12) | (7 << 16)) });
    // CPAR = TIM15.CCR1 register address.
    dp.DMA1
        .cpar5
        .write(|w| unsafe { w.bits(&dp.TIM15.ccr1 as *const _ as u32) });
    // CMAR = static u32[32] buffer in SRAM.
    let buf_addr = core::ptr::addr_of!(DSHOT_BUF) as u32;
    dp.DMA1.cmar5.write(|w| unsafe { w.bits(buf_addr) });
    // CNDTR = 32 transfers (32 edges captured per DSHOT frame slot).
    dp.DMA1.cndtr5.write(|w| unsafe { w.bits(32) });
    // CCR = 0x098B: EN+TCIE+TEIE+MINC+PSIZE=16bit+MSIZE=32bit, PL=low, M2P=0 (peri→mem).
    dp.DMA1.ccr5.write(|w| unsafe { w.bits(0x098B) });

    // --- TIM15 enable (CR1.CEN=1) — last, after capture chain is armed ---
    dp.TIM15.cr1.write(|w| unsafe { w.bits(1) });

    // --- TIM1 (motor PWM, advanced timer @ 80 MHz / 3333 ≈ 24 kHz carrier) ---
    // PSC=0, ARR=0xD04 (3332). CCMR=0x6868 = PWM1 + OCxPE preload on all 4 ch.
    // CCER=0x1555 = CCxE + CCxNE for ch1/2/3, CC4E only. BDTR=0xA02D =
    // MOE + OSSR + DTG=0x2D (~560 ns dead-time @ 80 MHz). CCR4=0x64 (trigger).
    dp.TIM1.psc.write(|w| unsafe { w.bits(0) });
    dp.TIM1.arr.write(|w| unsafe { w.bits(0x0D04) });
    dp.TIM1.ccmr1_output().write(|w| unsafe { w.bits(0x6868) });
    dp.TIM1.ccmr2_output().write(|w| unsafe { w.bits(0x6868) });
    dp.TIM1.ccer.write(|w| unsafe { w.bits(0x1555) });
    dp.TIM1.ccr4.write(|w| unsafe { w.bits(0x64) });
    dp.TIM1.bdtr.write(|w| unsafe { w.bits(0xA02D) });
    // CR1 = ARPE + CEN. Write last to start the timer.
    dp.TIM1.cr1.write(|w| unsafe { w.bits(0x81) });

    // --- TIM2 (general-purpose free-running 2 MHz μs timer) ---
    dp.TIM2.psc.write(|w| unsafe { w.bits(0x27) });
    dp.TIM2.arr.write(|w| unsafe { w.bits(0xFFFF) });
    dp.TIM2.cr1.write(|w| unsafe { w.bits(1) });

    // --- TIM6 (system tick @ ~19.6 kHz: 80 MHz / 80 / 51) ---
    // PSC=0x4F=79 → 1 MHz tick. ARR=0x32=50 → period 51. DIER.UIE=1.
    dp.TIM6.psc.write(|w| unsafe { w.bits(0x4F) });
    dp.TIM6.arr.write(|w| unsafe { w.bits(0x32) });
    dp.TIM6.dier.write(|w| w.uie().set_bit());
    dp.TIM6.cr1.write(|w| unsafe { w.bits(1) });

    // --- TIM7 (secondary 1 MHz μs counter, free-running 16-bit) ---
    dp.TIM7.psc.write(|w| unsafe { w.bits(0x4F) });
    dp.TIM7.arr.write(|w| unsafe { w.bits(0xFFFF) });
    dp.TIM7.cr1.write(|w| unsafe { w.bits(1) });

    // --- TIM16 (commutation timer @ 2 MHz tick) ---
    dp.TIM16.psc.write(|w| unsafe { w.bits(0x27) });
    dp.TIM16.arr.write(|w| unsafe { w.bits(0xFFFF) });
    dp.TIM16.cr1.write(|w| unsafe { w.bits(0x81) });

    // --- DMA1 CH4 (USART1_TX path). Channel disabled until USART1 TX wants it. ---
    // AM32 leaves CCR4=0x9A pre-configured: 0b0000_1001_1010
    //   bit 1 TCIE=1, bit 3 TEIE=1, bit 4 DIR=1 (mem→peri), bit 7 MINC=1.
    // PSIZE/MSIZE both 8-bit (zero). EN=0 — channel idle.
    // CPAR = USART1.TDR address. CMAR points at AM32's TX buffer; ours unused.
    dp.DMA1.ccr4.write(|w| unsafe { w.bits(0x9A) });
    dp.DMA1
        .cpar4
        .write(|w| unsafe { w.bits(&dp.USART1.tdr as *const _ as u32) });
    dp.DMA1
        .cmar4
        .write(|w| unsafe { w.bits(core::ptr::addr_of!(USART_TX_BUF) as u32) });

    // --- USART1 (half-duplex 115200 8N1 on PB6, DMA TX enabled, RX active) ---
    // BRR = 80 MHz / 115200 ≈ 694 = 0x2B6.
    // CR3 = HDSEL (bit 3) + DMAT (bit 7) = 0x88.
    // CR1 = UE (bit 0) + RE (bit 2) = 0x05. NO TE (matches AM32 idle config —
    // AM32 enables TE only when about to transmit, leaves RE on for passthrough).
    dp.USART1.brr.write(|w| unsafe { w.bits(0x2B6) });
    dp.USART1.cr3.write(|w| unsafe { w.bits(0x88) });
    dp.USART1.cr1.write(|w| unsafe { w.bits(0x05) });

    // --- IWDG (watchdog: PR=2 → /16 → 2 kHz tick, RLR=0xFA0 → ~2 sec timeout) ---
    // Unlock with KR=0x5555, write PR + RLR, then start with KR=0xCCCC. IWDG MUST
    // be started for PR/RLR to commit to the LSI clock domain — otherwise the
    // sync flags (PVU/RVU) never clear and the registers read as reset values.
    // After start, KR=0xAAAA must be written periodically (in the spin loop) to
    // refresh; otherwise chip resets every 2 s.
    dp.IWDG.kr.write(|w| unsafe { w.bits(0x5555) });
    dp.IWDG.pr.write(|w| unsafe { w.bits(2) });
    dp.IWDG.rlr.write(|w| unsafe { w.bits(0xFA0) });
    dp.IWDG.kr.write(|w| unsafe { w.bits(0xCCCC) }); // start watchdog
    while dp.IWDG.sr.read().bits() != 0 {} // wait for PR/RLR commit to LSI

    // --- COMP2 (BEMF zero-crossing) ---
    // AM32 dump: CSR = 0x40000071. Decode (per L431 RM):
    //   bit 0  EN = 1
    //   bit 4-6 INMSEL = 0b111 (extended — bits 25/26 = INMESEL select PB3)
    //   bit 30 LOCK = 1
    // Write config first (no LOCK), then LOCK separately — single-write with
    // LOCK=1 may have the LOCK latch before the rest of the word commits.
    let comp = unsafe { &*pac::COMP::PTR };
    comp.comp2_csr.write(|w| unsafe { w.bits(0x0000_0071) });
    comp.comp2_csr.write(|w| unsafe { w.bits(0x4000_0071) });

    // Spin loop refreshes IWDG via KR=0xAAAA so we don't watchdog-reset before
    // the user can dump registers. SWD AHB-AP still reaches RCC/peripherals
    // because we're not in WFI.
    loop {
        dp.IWDG.kr.write(|w| unsafe { w.bits(0xAAAA) });
        for _ in 0..1000 {
            cortex_m::asm::nop();
        }
    }
}

#[unsafe(link_section = ".bss")]
static mut DSHOT_BUF: [u32; 32] = [0; 32];

#[unsafe(link_section = ".bss")]
static mut USART_TX_BUF: [u8; 64] = [0; 64];
