//! System-memory life-probe: factory UID, flash size, package code, ADC/
//! temp-sensor factory calibration words, and DBGMCU IDCODE, all read from
//! the die and printed over RTT.
//!
//! Run: `cargo run --release --example chip-info`

#![no_std]
#![no_main]

use binz as _; // panic handler
use core::ptr;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    let _rcc = dp.RCC.freeze(Config::pll());

    unsafe {
        let uid0 = ptr::read_volatile(0x1FFF_7590 as *const u32);
        let uid1 = ptr::read_volatile(0x1FFF_7594 as *const u32);
        let uid2 = ptr::read_volatile(0x1FFF_7598 as *const u32);
        let flash_kb = ptr::read_volatile(0x1FFF_75E0 as *const u16);
        let package = ptr::read_volatile(0x1FFF_7500 as *const u16) & 0xF;
        let ts_cal1 = ptr::read_volatile(0x1FFF_75A8 as *const u16);
        let vrefint_cal = ptr::read_volatile(0x1FFF_75AA as *const u16);
        let idcode = ptr::read_volatile(0x4001_5800 as *const u32);

        rprintln!("chip-info (STM32G071 system memory):");
        rprintln!("  UID       = {:08x}-{:08x}-{:08x}", uid2, uid1, uid0);
        rprintln!("  flash     = {} KB", flash_kb);
        rprintln!(
            "  package   = {:#x} (0xc observed on this LQFP64 part)",
            package
        );
        rprintln!("  TS_CAL1   = {} (ADC raw @30C/3.0V)", ts_cal1);
        rprintln!("  VREFINT   = {} (ADC raw @3.0V)", vrefint_cal);
        rprintln!(
            "  IDCODE    = {:#010x} (DEV_ID={:#05x}, REV={:#06x})",
            idcode,
            idcode & 0xFFF,
            idcode >> 16
        );
        let plausible = (idcode & 0xFFF) == 0x460 && flash_kb == 128 && ts_cal1 > 0;
        rprintln!(
            "SYSTEM MEMORY: {}",
            if plausible { "ALIVE - PASS" } else { "FAIL" }
        );
    }

    loop {
        cortex_m::asm::nop();
    }
}
