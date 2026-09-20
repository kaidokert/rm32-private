//! CRC peripheral life-probe: known-answer test against standard CRC-32
//! ("123456789" -> 0xCBF43926 after output XOR, byte-reversed config).
//!
//! Run: `cargo run --release --example crc32`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::crc::{BitReversal, CrcExt};
use stm32g0xx_hal::rcc::RccExt;
use stm32g0xx_hal::stm32;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.constrain();

    let crc = dp.CRC.constrain(&mut rcc);
    let mut crc = crc
        .input_bit_reversal(Some(BitReversal::ByWord))
        .output_bit_reversal(true)
        .freeze();

    crc.reset();
    crc.feed(b"123456789");
    let r1 = crc.result() ^ 0xffff_ffff;

    crc.reset();
    crc.feed(b"The quick brown fox jumps over the lazy dog");
    let r2 = crc.result() ^ 0xffff_ffff;

    let pass = r1 == 0xcbf4_3926 && r2 == 0x414f_a339;
    rprintln!("crc32(\"123456789\") = {:#010x} (want 0xcbf43926)", r1);
    rprintln!("crc32(quick brown fox) = {:#010x} (want 0x414fa339)", r2);
    rprintln!(
        "CRC PERIPHERAL: {}",
        if pass { "ALIVE - PASS" } else { "FAIL" }
    );

    loop {
        cortex_m::asm::nop();
    }
}
