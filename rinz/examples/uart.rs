#![deny(warnings)]
#![deny(unsafe_code)]
#![no_main]
#![no_std]
#![allow(clippy::uninlined_format_args)]

use embedded_io::{Read, Write};
use hal::prelude::*;
use hal::pwr::PwrExt;
use hal::serial::FullConfig;
use hal::{rcc, stm32};
use rinz::hal;

use cortex_m_rt::entry;

#[entry]
fn main() -> ! {
    rinz::panic::ensure_rtt();
    rtt_target::rprintln!("start");

    let dp = stm32::Peripherals::take().expect("cannot take peripherals");
    let pwr = dp.PWR.constrain().freeze();
    let mut rcc = dp.RCC.freeze(rcc::Config::hsi(), pwr);

    rtt_target::rprintln!("Init UART");
    let gpiob = dp.GPIOB.split(&mut rcc);

    // PB3=TX, PB4=RX on USART2 (AF7)
    let tx = gpiob.pb3.into_alternate();
    let rx = gpiob.pb4.into_alternate();
    let mut usart = dp
        .USART2
        .usart(tx, rx, FullConfig::default(), &mut rcc)
        .unwrap();

    usart.write_all(b"Hello USART3!\r\n").unwrap();
    rtt_target::rprintln!("Hello USART3!");

    let mut read_buf = [0u8; 8];
    usart.read_exact(&mut read_buf).unwrap();
    usart.write_all(&read_buf).unwrap();

    let mut single_byte_buffer = [0; 1];
    let mut cnt = 0u32;
    loop {
        match usart.read_exact(&mut single_byte_buffer) {
            Ok(()) => {
                rtt_target::rprintln!("{}: {}", cnt, single_byte_buffer[0]);
                usart.write_all(&single_byte_buffer).unwrap();
            }
            Err(_e) => {
                rtt_target::rprintln!("read error");
            }
        };
        cnt += 1;
    }
}
