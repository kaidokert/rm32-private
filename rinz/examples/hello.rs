#![no_std]
#![no_main]

use rinz::hal as _;

#[cortex_m_rt::entry]
fn main() -> ! {
    rinz::panic::ensure_rtt();
    loop {
        cortex_m::asm::delay(10_000_000);
        rtt_target::rprintln!("Hello, world!");
    }
}
