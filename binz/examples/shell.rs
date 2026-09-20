//! Interactive bench shell over USART3 (PC10/PC11, 115200) for the DRV8304 rig.
//! Read every ADC channel, read every GPIO input level, toggle both LEDs, and
//! individually drive every output pin we care about. Boot-safe: gates + ENABLE low.
//!
//! Commands (newline-terminated):
//!   ?            help
//!   a            dump the wired ADC channels (VDDA, VBUS, 3x current, VSENC, neutral)
//!   r <n>        read one ADC channel n (0..18) raw + mV
//!   c            BEMF comparators (COMP2 vs neutral) for A/B/C
//!   i            dump input LEVEL (IDR) of every pin incl nFAULT + button
//!   p            dump output-DRIVE state (ODR) of the output pins
//!   blink        toggle antiphase LD4/LED blink
//!   off          all gates + ENABLE low (safe)
//!   <pin>=<0|1>  set an output. pins: ah bh ch (highs) al bl cl (lows) led ld4 en
//! e.g.  en=1   ah=1   bl=1   led=0   r 12   i   off
//!
//! Map: ah=PA10 bh=PA9 ch=PA8 | al=PB1 bl=PB0 cl=PA7 | led=PB5 ld4=PA5 en=PD1
//!      nflt=PB14 (in)  btn=PC13 (in)
//! Run: cargo run --release --example shell

#![no_std]
#![no_main]

use binz as _; // panic handler
use core::fmt::Write;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::BasicConfig;
use stm32g0xx_hal::stm32;

const VREFINT_CAL_ADDR: u32 = 0x1FFF_75AA;
const COMP2_CSR: *mut u32 = 0x4001_0204 as *mut u32;

unsafe fn adc_read(ch: u8) -> u16 {
    let adc = &*stm32::ADC::ptr();
    adc.isr().write(|w| w.bits(1 << 13));
    adc.chselr0().write(|w| w.bits(1 << ch));
    while adc.isr().read().bits() & (1 << 13) == 0 {}
    adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
    while adc.isr().read().bits() & (1 << 2) == 0 {}
    adc.dr().read().bits() as u16
}

// port: 0=A 1=B 2=C 3=D
fn set_pin(port: u8, bit: u32, on: bool) {
    let v = if on { 1 << bit } else { 1 << (bit + 16) };
    unsafe {
        match port {
            0 => (*stm32::GPIOA::ptr()).bsrr().write(|w| w.bits(v)),
            1 => (*stm32::GPIOB::ptr()).bsrr().write(|w| w.bits(v)),
            2 => (*stm32::GPIOC::ptr()).bsrr().write(|w| w.bits(v)),
            _ => (*stm32::GPIOD::ptr()).bsrr().write(|w| w.bits(v)),
        };
    }
}
fn get_odr(port: u8, bit: u32) -> bool {
    unsafe {
        let r = match port {
            0 => (*stm32::GPIOA::ptr()).odr().read().bits(),
            1 => (*stm32::GPIOB::ptr()).odr().read().bits(),
            2 => (*stm32::GPIOC::ptr()).odr().read().bits(),
            _ => (*stm32::GPIOD::ptr()).odr().read().bits(),
        };
        r & (1 << bit) != 0
    }
}
fn get_idr(port: u8, bit: u32) -> bool {
    unsafe {
        let r = match port {
            0 => (*stm32::GPIOA::ptr()).idr().read().bits(),
            1 => (*stm32::GPIOB::ptr()).idr().read().bits(),
            2 => (*stm32::GPIOC::ptr()).idr().read().bits(),
            _ => (*stm32::GPIOD::ptr()).idr().read().bits(),
        };
        r & (1 << bit) != 0
    }
}
fn pin_lookup(name: &[u8]) -> Option<(u8, u32)> {
    match name {
        b"ah" => Some((0, 10)),
        b"bh" => Some((0, 9)),
        b"ch" => Some((0, 8)),
        b"al" => Some((1, 1)),
        b"bl" => Some((1, 0)),
        b"cl" => Some((0, 7)),
        b"led" => Some((1, 5)),
        b"ld4" => Some((0, 5)),
        b"en" => Some((3, 1)),
        _ => None,
    }
}
// Every pin we care to read a level from (outputs + the two inputs).
const ALL_PINS: [(&str, u8, u32); 11] = [
    ("ah", 0, 10),
    ("bh", 0, 9),
    ("ch", 0, 8),
    ("al", 1, 1),
    ("bl", 1, 0),
    ("cl", 0, 7),
    ("led", 1, 5),
    ("ld4", 0, 5),
    ("en", 3, 1),
    ("nflt", 1, 14),
    ("btn", 2, 13),
];
fn comp_read(code: u32) -> bool {
    unsafe {
        let v = core::ptr::read_volatile(COMP2_CSR);
        core::ptr::write_volatile(
            COMP2_CSR,
            (v & !(0xF << 4 | 0x3 << 8)) | (code << 4) | (0b10 << 8),
        );
    }
    cortex_m::asm::delay(3000);
    unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    // Analog: currents PA0/PA1/PA4, VSENC PA2, neutral PA3, VBUS PA6, BEMF-A/B PB3/PB7.
    let _ = (
        gpioa.pa0.into_analog(),
        gpioa.pa1.into_analog(),
        gpioa.pa2.into_analog(),
        gpioa.pa3.into_analog(),
        gpioa.pa4.into_analog(),
        gpioa.pa6.into_analog(),
        gpiob.pb3.into_analog(),
        gpiob.pb7.into_analog(),
    );
    // Inputs: nFAULT PB14 (EVM pull-up), button PC13.
    let _ = (
        gpiob.pb14.into_floating_input(),
        gpioc.pc13.into_floating_input(),
    );
    // Outputs (driven low below).
    let _ = (
        gpioa.pa5.into_push_pull_output(),
        gpioa.pa7.into_push_pull_output(),
        gpioa.pa8.into_push_pull_output(),
        gpioa.pa9.into_push_pull_output(),
        gpioa.pa10.into_push_pull_output(),
        gpiob.pb0.into_push_pull_output(),
        gpiob.pb1.into_push_pull_output(),
        gpiob.pb5.into_push_pull_output(),
        gpiod.pd1.into_push_pull_output(),
    );
    for (p, b) in [
        (0, 5),
        (0, 7),
        (0, 8),
        (0, 9),
        (0, 10),
        (1, 0),
        (1, 1),
        (1, 5),
        (3, 1),
    ] {
        set_pin(p, b, false);
    }

    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();

    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw.apbenr2().modify(|r, w| w.bits(r.bits() | (1 << 0))); // SYSCFGEN
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 20))); // ADCEN
        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30));
        adc.cr().write(|w| w.bits(1 << 28));
        cortex_m::asm::delay(64 * 30);
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31)));
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        adc.smpr().write(|w| w.bits(0b111));
        adc.ccr().modify(|r, w| w.bits(r.bits() | (1 << 22))); // VREFEN
        adc.isr().write(|w| w.bits(1));
        adc.cr().modify(|r, w| w.bits(r.bits() | 1));
        while adc.isr().read().bits() & 1 == 0 {}
        core::ptr::write_volatile(COMP2_CSR, (0b1000 << 4) | (0b10 << 8) | 1);
        cortex_m::asm::delay(320);
    }
    let vcal = unsafe { core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) } as u32;

    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(64_000 - 1);
    syst.clear_current();
    syst.enable_counter();

    let _ = writeln!(serial, "\r\n=== DRV8304 bench shell ===  type ? for help");
    while serial.flush().is_err() {}
    let _ = write!(serial, "> ");
    while serial.flush().is_err() {}
    rprintln!("shell ready");

    let mut buf = [0u8; 32];
    let mut idx = 0usize;
    let mut millis: u32 = 0;
    let mut last_blink: u32 = 0;
    let mut blink = true;
    let mut bphase = false;

    loop {
        if syst.has_wrapped() {
            millis = millis.wrapping_add(1);
        }
        if blink && millis.wrapping_sub(last_blink) >= 500 {
            last_blink = millis;
            bphase = !bphase;
            set_pin(0, 5, bphase); // LD4
            set_pin(1, 5, !bphase); // DRV LED antiphase
        }

        if let Ok(b) = serial.read() {
            if b == b'\r' || b == b'\n' {
                let line = &buf[..idx];
                let _ = writeln!(serial, "");
                if idx == 0 {
                    // nothing
                } else if line == b"?" || line == b"help" {
                    let _ = writeln!(
                        serial,
                        "a=adc r<n>=1ch c=comp i=inputs p=outputs blink off  pin=0/1: ah bh ch al bl cl led ld4 en"
                    );
                } else if line == b"a" {
                    let vref = unsafe { adc_read(13) } as u32;
                    let vdda = if vref > 0 { 3000 * vcal / vref } else { 0 };
                    let mv = |raw: u16| raw as u32 * vdda / 4096;
                    let ia = mv(unsafe { adc_read(0) });
                    let ib = mv(unsafe { adc_read(1) });
                    let ic = mv(unsafe { adc_read(4) });
                    let vbus = mv(unsafe { adc_read(6) }) * 1194 / 100;
                    let vsc = mv(unsafe { adc_read(2) });
                    let neu = mv(unsafe { adc_read(3) });
                    let off = (vdda / 2) as i32;
                    let ma = |m: u32| (m as i32 - off) * 1000 / 70;
                    let _ = writeln!(serial, "VDDA={}mV VBUS={}mV", vdda, vbus);
                    let _ = writeln!(
                        serial,
                        "IA={}mV({}mA) IB={}mV({}mA) IC={}mV({}mA)",
                        ia,
                        ma(ia),
                        ib,
                        ma(ib),
                        ic,
                        ma(ic)
                    );
                    let _ = writeln!(serial, "VSENC(PA2)={}mV NEU(PA3)={}mV", vsc, neu);
                } else if line[0] == b'r' {
                    // read one channel: "r <n>"
                    let mut n: u32 = 0;
                    let mut any = false;
                    for &c in &line[1..] {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c - b'0') as u32;
                            any = true;
                        }
                    }
                    if any && n <= 18 {
                        let vref = unsafe { adc_read(13) } as u32;
                        let vdda = if vref > 0 { 3000 * vcal / vref } else { 0 };
                        let raw = unsafe { adc_read(n as u8) };
                        let _ = writeln!(
                            serial,
                            "ch{} raw={} {}mV (VDDA={})",
                            n,
                            raw,
                            raw as u32 * vdda / 4096,
                            vdda
                        );
                    } else {
                        let _ = writeln!(serial, "?r <0..18>");
                    }
                } else if line == b"c" {
                    let s = |x: bool| if x { "hi" } else { "lo" };
                    let _ = writeln!(
                        serial,
                        "BEMF vs neutral: A(PB3)={} B(PB7)={} C(PA2)={}",
                        s(comp_read(6)),
                        s(comp_read(7)),
                        s(comp_read(8))
                    );
                } else if line == b"i" {
                    let _ = write!(serial, "IN:");
                    for (nm, p, b) in ALL_PINS {
                        let _ = write!(serial, " {}={}", nm, if get_idr(p, b) { 1 } else { 0 });
                    }
                    let _ = writeln!(serial, "");
                } else if line == b"p" {
                    let g = |p, b| if get_odr(p, b) { '1' } else { '0' };
                    let _ = writeln!(
                        serial,
                        "OUT: ah={} bh={} ch={} | al={} bl={} cl={} | led={} ld4={} en={}",
                        g(0, 10),
                        g(0, 9),
                        g(0, 8),
                        g(1, 1),
                        g(1, 0),
                        g(0, 7),
                        g(1, 5),
                        g(0, 5),
                        g(3, 1)
                    );
                } else if line == b"blink" {
                    blink = !blink;
                    let _ = writeln!(serial, "blink {}", if blink { "on" } else { "off" });
                } else if line == b"off" {
                    for (p, b) in [(0, 7), (0, 8), (0, 9), (0, 10), (1, 0), (1, 1), (3, 1)] {
                        set_pin(p, b, false);
                    }
                    let _ = writeln!(serial, "gates + en OFF");
                } else if let Some(eq) = line.iter().position(|&c| c == b'=') {
                    let name = &line[..eq];
                    let val = line.get(eq + 1).copied().unwrap_or(b'0');
                    let on = val == b'1' || val == b'h';
                    match pin_lookup(name) {
                        Some((p, b)) => {
                            if name == b"led" || name == b"ld4" {
                                blink = false;
                            }
                            set_pin(p, b, on);
                            let _ = writeln!(
                                serial,
                                "{}={}",
                                core::str::from_utf8(name).unwrap_or("?"),
                                if on { 1 } else { 0 }
                            );
                        }
                        None => {
                            let _ = writeln!(serial, "?pin");
                        }
                    }
                } else {
                    let _ = writeln!(serial, "?cmd (try ?)");
                }
                idx = 0;
                let _ = write!(serial, "> ");
                while serial.flush().is_err() {}
            } else if idx < buf.len() - 1 && b >= 0x20 {
                buf[idx] = b;
                idx += 1;
                let _ = serial.write(b); // echo
            }
        }
    }
}
