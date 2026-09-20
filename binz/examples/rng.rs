//! RNG life-probe -- UNDOCUMENTED HARDWARE. ST documents no RNG on the
//! STM32G071 (it's a G081 feature), and both the PAC and HAL omit it.
//! But the G071/G081 share a die: SWD probing found a live TRNG at the
//! G0 RNG address (0x40025000) that produces entropy once RCC feeds it
//! HSI16 (CCIPR.RNGSEL=01) and the AHB clock gate opens (AHBENR bit 18).
//! Raw register access, since no crate knows this peripheral exists here.
//!
//! Life criteria: DRDY sets, 32 samples are pairwise-distinct, and the
//! ones-density across 1024 bits lands near 50%.
//!
//! Run: `cargo run --release --example rng`

#![no_std]
#![no_main]

use binz as _; // panic handler
use core::ptr::{read_volatile, write_volatile};
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

const RCC_AHBENR: *mut u32 = 0x4002_1038 as *mut u32;
const RCC_CCIPR: *mut u32 = 0x4002_1054 as *mut u32;
const RNG_CR: *mut u32 = 0x4002_5000 as *mut u32;
const RNG_SR: *mut u32 = 0x4002_5004 as *mut u32;
const RNG_DR: *mut u32 = 0x4002_5008 as *mut u32;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    let _rcc = dp.RCC.freeze(Config::pll());

    unsafe {
        // Kernel clock: HSI16 (RNGSEL=01, RNGDIV=1). RMW to preserve other
        // CCIPR kernel-clock selections.
        write_volatile(RCC_CCIPR, read_volatile(RCC_CCIPR) | (0b01 << 26));
        write_volatile(RCC_AHBENR, read_volatile(RCC_AHBENR) | (1 << 18));
        write_volatile(RNG_CR, 1 << 2); // RNGEN
        write_volatile(RNG_SR, 0); // clear stale CEIS/SEIS

        let mut samples = [0u32; 32];
        for s in samples.iter_mut() {
            let mut spins = 0u32;
            while read_volatile(RNG_SR) & 1 == 0 {
                spins += 1;
                if spins > 10_000_000 {
                    rprintln!("RNG FAIL: DRDY never set (SR={:#x})", read_volatile(RNG_SR));
                    loop {
                        cortex_m::asm::nop();
                    }
                }
            }
            *s = read_volatile(RNG_DR);
            // Clear the startup CEIS latch (clock error before HSI16 kernel
            // clock settled) once entropy is flowing; only current-status
            // bits matter for the verdict below.
            write_volatile(RNG_SR, 0);
        }
        let sr = read_volatile(RNG_SR);

        let mut distinct = true;
        for i in 0..samples.len() {
            for j in i + 1..samples.len() {
                if samples[i] == samples[j] {
                    distinct = false;
                }
            }
        }
        let ones: u32 = samples.iter().map(|w| w.count_ones()).sum();
        rprintln!("rng (undocumented on G071): SR={:#x}", sr);
        rprintln!(
            "first words: {:08x} {:08x} {:08x} {:08x}",
            samples[0],
            samples[1],
            samples[2],
            samples[3]
        );
        rprintln!(
            "32 samples: distinct={} ones={}/1024 (want ~512)",
            distinct,
            ones
        );
        // CECS (bit 1) / SECS (bit 2) are the *current* clock/seed error
        // status; sticky IS latches are informational only.
        let pass = distinct && (384..=640).contains(&ones) && sr & 0b110 == 0;
        rprintln!("HIDDEN RNG: {}", if pass { "ALIVE - PASS" } else { "FAIL" });
    }

    loop {
        cortex_m::asm::nop();
    }
}
