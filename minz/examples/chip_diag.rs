//! Minimal chip diagnostic — no board_init, no ISRs, no HAL clock setup.
//! Runs on the default MSI 4 MHz reset clock. Dumps key peripheral registers
//! via RTT so we can see the chip state without anything fancy running.
//!
//! Flash and attach RTT console:
//!   cargo run --release --example chip_diag --target thumbv7em-none-eabihf

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use minz::hal::stm32::{COMP, GPIOA, GPIOB, RCC, TIM1};
use rtt_target::rprintln;

// Bare pointer reads — we're not using the HAL here, just dumping raw values.
unsafe fn r32(addr: u32) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

#[entry]
fn main() -> ! {
    minz::panic::ensure_rtt();
    rprintln!("=== chip_diag start ===");

    // Let the probe settle; a few nops give RTT the chance to drain.
    for _ in 0..100_000u32 {
        cortex_m::asm::nop();
    }

    dump_rcc();
    dump_gpioa();
    dump_gpiob();
    dump_tim1();
    dump_comp();
    dump_nvic();
    dump_scb();

    rprintln!("=== dump done, looping ===");
    let mut n: u32 = 0;
    loop {
        for _ in 0..400_000u32 {
            cortex_m::asm::nop();
        }
        n = n.wrapping_add(1);
        rprintln!("alive {}", n);
    }
}

fn dump_rcc() {
    let rcc = unsafe { &*RCC::ptr() };
    let cr = unsafe { r32(0x4002_1000) }; // RCC_CR
    let cfgr = unsafe { r32(0x4002_1008) }; // RCC_CFGR
    let csr = unsafe { r32(0x4002_1094) }; // RCC_CSR (reset flags)
    let ahb2 = unsafe { r32(0x4002_104C) }; // RCC_AHB2ENR
    let apb2 = unsafe { r32(0x4002_1060) }; // RCC_APB2ENR
    rprintln!("RCC CR=0x{:08x}  CFGR=0x{:08x}", cr, cfgr);
    rprintln!(
        "    CSR=0x{:08x}  AHB2ENR=0x{:08x}  APB2ENR=0x{:08x}",
        csr,
        ahb2,
        apb2
    );
    // CFGR SWS field [3:2]: 00=MSI, 01=HSI16, 10=HSE, 11=PLL
    let sws = (cfgr >> 2) & 0x3;
    rprintln!("    SWS={} (0=MSI 1=HSI16 2=HSE 3=PLL)", sws);
    let _ = rcc;
}

fn dump_gpioa() {
    let base = 0x4800_0000u32; // GPIOA
    let moder = unsafe { r32(base + 0x00) };
    let otyper = unsafe { r32(base + 0x04) };
    let ospeedr = unsafe { r32(base + 0x08) };
    let pupdr = unsafe { r32(base + 0x0C) };
    let idr = unsafe { r32(base + 0x10) };
    let afrl = unsafe { r32(base + 0x20) };
    let afrh = unsafe { r32(base + 0x24) };
    rprintln!(
        "GPIOA MODER=0x{:08x} OTYPER=0x{:04x} PUPDR=0x{:08x}",
        moder,
        otyper,
        pupdr
    );
    rprintln!("      OSPEEDR=0x{:08x} IDR=0x{:04x}", ospeedr, idr);
    rprintln!("      AFRL=0x{:08x} AFRH=0x{:08x}", afrl, afrh);
    let _ = unsafe { &*GPIOA::ptr() };
}

fn dump_gpiob() {
    let base = 0x4800_0400u32; // GPIOB
    let moder = unsafe { r32(base + 0x00) };
    let pupdr = unsafe { r32(base + 0x0C) };
    let idr = unsafe { r32(base + 0x10) };
    let afrl = unsafe { r32(base + 0x20) };
    rprintln!(
        "GPIOB MODER=0x{:08x} PUPDR=0x{:08x} IDR=0x{:04x}",
        moder,
        pupdr,
        idr
    );
    rprintln!("      AFRL=0x{:08x}", afrl);
    let _ = unsafe { &*GPIOB::ptr() };
}

fn dump_tim1() {
    let base = 0x4001_2C00u32; // TIM1
    let cr1 = unsafe { r32(base + 0x00) };
    let cr2 = unsafe { r32(base + 0x04) };
    let smcr = unsafe { r32(base + 0x08) };
    let dier = unsafe { r32(base + 0x0C) };
    let sr = unsafe { r32(base + 0x10) };
    let ccer = unsafe { r32(base + 0x20) };
    let arr = unsafe { r32(base + 0x2C) };
    let bdtr = unsafe { r32(base + 0x44) };
    rprintln!(
        "TIM1  CR1=0x{:04x} CR2=0x{:04x} SMCR=0x{:08x}",
        cr1,
        cr2,
        smcr
    );
    rprintln!(
        "      DIER=0x{:08x} SR=0x{:08x} CCER=0x{:08x}",
        dier,
        sr,
        ccer
    );
    rprintln!("      ARR=0x{:08x} BDTR=0x{:08x}", arr, bdtr);
    let _ = unsafe { &*TIM1::ptr() };
}

fn dump_comp() {
    let base = 0x4001_0200u32; // COMP
    let csr2 = unsafe { r32(base + 0x04) }; // COMP2_CSR (offset 4 from COMP1_CSR)
    rprintln!("COMP  CSR2=0x{:08x}", csr2);
    let _ = unsafe { &*COMP::ptr() };
}

fn dump_nvic() {
    // NVIC IPR registers: each byte is one IRQ's priority.
    // IRQ numbers we care about on L431:
    //   COMP=21, TIM1_UP_TIM16=25, TIM1_CC=27(?), TIM7=55, LPTIM1=65
    // IPR register n covers IRQs [4n..4n+3], byte offset n/4*4+n%4.
    // Read the raw IPR words for the relevant ranges.
    let ipr_base = 0xE000_E400u32;
    // IPR[5] covers IRQ20-23 (COMP=21)
    let ipr5 = unsafe { r32(ipr_base + 5 * 4) };
    // IPR[6] covers IRQ24-27 (TIM1_UP_TIM16=25?, TIM1_CC=27?)
    let ipr6 = unsafe { r32(ipr_base + 6 * 4) };
    // IPR[13] covers IRQ52-55 (TIM7=55)
    let ipr13 = unsafe { r32(ipr_base + 13 * 4) };
    // IPR[16] covers IRQ64-67 (LPTIM1=65)
    let ipr16 = unsafe { r32(ipr_base + 16 * 4) };
    rprintln!("NVIC  IPR5=0x{:08x}  IPR6=0x{:08x}", ipr5, ipr6);
    rprintln!("      IPR13=0x{:08x} IPR16=0x{:08x}", ipr13, ipr16);
    rprintln!("      COMP(IRQ21) prio byte=0x{:02x}", (ipr5 >> 8) & 0xFF);
}

fn dump_scb() {
    let aircr = unsafe { r32(0xE000_ED0C) };
    let shpr3 = unsafe { r32(0xE000_ED20) }; // SysTick priority in bits [31:24]
    rprintln!(
        "SCB   AIRCR=0x{:08x} (PRIGROUP={}) SHPR3=0x{:08x}",
        aircr,
        (aircr >> 8) & 7,
        shpr3
    );
    rprintln!("      SysTick prio byte=0x{:02x}", (shpr3 >> 24) & 0xFF);
}
