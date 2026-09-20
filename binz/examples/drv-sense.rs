//! BEMF sense-wire test for the DRV8304 rig. Confirms the 4 sense wires:
//!   VSENA->PB3 (COMP2 INM IO1, code 6)   phase A
//!   VSENB->PB7 (COMP2 INM IO2, code 7)   phase B
//!   VSENC->PA2 (COMP2 INM IO3, code 8)   phase C   (also ADC IN2)
//!   neutral->PA3 (COMP2 INP IO3)                    (also ADC IN3)
//! PB3/PB7 are NOT ADC pins on G071 -- rm32 senses BEMF via COMP2 (analog
//! comparator vs the star neutral), matching mcu_g071/comp_init.rs exactly.
//!
//! Method: drive the proven open-loop six-step (drv-spin6x core), and each
//! commutation step mux COMP2 INM to the FLOATING phase, INP=PA3. Sample the
//! comparator output (COMP2_CSR bit30, =1 when neutral>phase) mid-PWM-ON. A
//! live sense wire is NOT stuck: as our blind commutation drifts against the
//! real rotor, that phase's comparator crosses neutral (we see both 0 and 1).
//! A dead wire => stuck output. Per-phase we report seen-low/seen-high and a
//! crossing count. All three showing L+H (and crossings) = all 4 wires good
//! (any clean crossing also proves neutral/PA3, the shared reference).
//!
//! Status over USART3 (PC10) @115200, each line stamped with a monotonic
//! seq (#N) so fresh-vs-buffered is unambiguous. NO current sense -> keep the
//! PSU current-limited. Run: cargo run --release --example drv-sense

#![no_std]
#![no_main]

use binz as _; // panic handler
use core::fmt::Write;
use cortex_m::asm::delay as cycdelay;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::BasicConfig;
use stm32g0xx_hal::stm32;

const DELAY_1US: u32 = 21;
const CARRIER_HZ: u32 = 16_000;
const PERIOD_UNITS: u32 = 62 * DELAY_1US;
const OVERHEAD_UNITS: u32 = 60;
const MIN_ON_UNITS: u32 = DELAY_1US / 2;

const INH_BIT: [u32; 3] = [10, 9, 8]; // A=PA10, B=PA9, C=PA8
const INL_PORT: [u8; 3] = [1, 1, 0]; // A=GPIOB, B=GPIOB, C=GPIOA
const INL_BIT: [u32; 3] = [1, 0, 7]; // A=PB1, B=PB0, C=PA7
const STEP: [(usize, usize); 6] = [(0, 1), (0, 2), (1, 2), (1, 0), (2, 0), (2, 1)];
// COMP2 INMSEL code per phase (from build.rs g071_comp2_inm): A=PB3=6, B=PB7=7, C=PA2=8.
const INMSEL: [u32; 3] = [6, 7, 8];

const COMP2_CSR: *mut u32 = 0x4001_0204 as *mut u32;

#[inline(always)]
fn bsrr_a(set: u32, clr: u32) {
    unsafe {
        (*stm32::GPIOA::ptr())
            .bsrr()
            .write(|w| w.bits(set | (clr << 16)));
    }
}
#[inline(always)]
fn bsrr_b(set: u32, clr: u32) {
    unsafe {
        (*stm32::GPIOB::ptr())
            .bsrr()
            .write(|w| w.bits(set | (clr << 16)));
    }
}
#[inline(always)]
fn set_inl(p: usize, high: bool) {
    let bit = 1u32 << INL_BIT[p];
    let (s, c) = if high { (bit, 0) } else { (0, bit) };
    if INL_PORT[p] == 0 {
        bsrr_a(s, c)
    } else {
        bsrr_b(s, c)
    }
}
#[inline(always)]
fn comp_set_inm(code: u32) {
    unsafe {
        let v = core::ptr::read_volatile(COMP2_CSR);
        let cleared = v & !(0xF << 4 | 0x3 << 8);
        core::ptr::write_volatile(COMP2_CSR, cleared | (code << 4) | (0b10 << 8)); // INP=IO3(PA3)
    }
}
#[inline(always)]
fn comp_out() -> bool {
    unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 }
}

/// Apply INL enables (6x diode mode: src INL low, sink INL high, float Hi-Z),
/// park INH low, mux COMP2 to the floating phase. Returns (src, float).
fn commutate(step: usize) -> (usize, usize) {
    let (src, sink) = STEP[step];
    let flt = 3 - src - sink;
    bsrr_a(0, (1 << INH_BIT[0]) | (1 << INH_BIT[1]) | (1 << INH_BIT[2]));
    set_inl(src, false);
    set_inl(sink, true);
    set_inl(flt, false);
    comp_set_inm(INMSEL[flt]);
    (src, flt)
}

fn all_off() {
    bsrr_a(
        0,
        (1 << 7) | (1 << INH_BIT[0]) | (1 << INH_BIT[1]) | (1 << INH_BIT[2]),
    );
    bsrr_b(0, (1 << 0) | (1 << 1));
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);

    let _ = (
        gpioa.pa7.into_push_pull_output(),
        gpioa.pa8.into_push_pull_output(),
        gpioa.pa9.into_push_pull_output(),
        gpioa.pa10.into_push_pull_output(),
        gpiob.pb0.into_push_pull_output(),
        gpiob.pb1.into_push_pull_output(),
    );
    all_off();

    // COMP2 setup (mirrors mcu_g071/comp_init.rs): SYSCFGEN, analog pins,
    // INMSEL=IO3(PA2) init, INPSEL=IO3(PA3), EN.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw.apbenr2().modify(|r, w| w.bits(r.bits() | 1)); // SYSCFGEN
        let ga = &*stm32::GPIOA::ptr();
        ga.moder()
            .modify(|r, w| w.bits(r.bits() | (0b11 << 4) | (0b11 << 6))); // PA2,PA3 analog
        let gb = &*stm32::GPIOB::ptr();
        gb.moder()
            .modify(|r, w| w.bits(r.bits() | (0b11 << 6) | (0b11 << 14))); // PB3,PB7 analog
        core::ptr::write_volatile(COMP2_CSR, (0b1000 << 4) | (0b10 << 8) | 1); // INM=PA2, INP=PA3, EN
        cycdelay(320); // ~5 us startup
    }

    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();
    let _ = writeln!(
        serial,
        "\r\ndrv-sense: COMP2 BEMF sense-wire test. A=PB3 B=PB7 C=PA2 neutral=PA3."
    );
    while serial.flush().is_err() {}
    rprintln!("drv-sense start");

    let (mut src, mut flt) = commutate(0);
    delay.delay(300.millis());

    let total_p = CARRIER_HZ * 13;
    let mut period: u32 = 0;
    let mut step: usize = 0;
    let mut since_comm: u32 = 0;
    let mut seq: u32 = 0;

    // Per-phase comparator observations (index 0=A,1=B,2=C).
    let mut seen_lo = [false; 3];
    let mut seen_hi = [false; 3];
    let mut cross = [0u32; 3];
    let mut last = [false; 3];

    loop {
        period += 1;
        // Ramp 5 -> 50 eHz over 5 s, then hold 50 (strong BEMF).
        let ehz = if period <= CARRIER_HZ * 5 {
            5 + 45 * period / (CARRIER_HZ * 5)
        } else {
            50
        };
        let duty_pct = core::cmp::min(350 + 15 * ehz, 1200);
        let duty_q16 = duty_pct * 65536 / 10000;
        let mut on_units = duty_q16 * PERIOD_UNITS >> 16;
        if on_units < MIN_ON_UNITS {
            on_units = 0;
        }

        let step_len = CARRIER_HZ / (6 * ehz);
        since_comm += 1;
        if since_comm >= step_len {
            since_comm = 0;
            step = (step + 1) % 6;
            let c = commutate(step);
            src = c.0;
            flt = c.1;
        }

        let inh = 1u32 << INH_BIT[src];
        if on_units > 0 {
            bsrr_a(inh, 0);
            cycdelay(on_units);
            // Sample comparator mid/late-ON (>2 periods after mux switch = settled).
            if since_comm > 2 {
                let c = comp_out();
                if c {
                    seen_hi[flt] = true
                } else {
                    seen_lo[flt] = true
                }
                if c != last[flt] {
                    cross[flt] += 1;
                    last[flt] = c;
                }
            }
            bsrr_a(0, inh);
        }
        cycdelay(PERIOD_UNITS.saturating_sub(on_units + OVERHEAD_UNITS));

        if period % (CARRIER_HZ / 2) == 0 {
            seq += 1;
            let tag = |p: usize| -> &'static str {
                match (seen_lo[p], seen_hi[p]) {
                    (true, true) => "LH",
                    (true, false) => "L.",
                    (false, true) => ".H",
                    (false, false) => "..",
                }
            };
            let _ = writeln!(
                serial,
                "#{} eHz={} | A(PB3) {} x{} | B(PB7) {} x{} | C(PA2) {} x{}",
                seq,
                ehz,
                tag(0),
                cross[0],
                tag(1),
                cross[1],
                tag(2),
                cross[2]
            );
            while serial.flush().is_err() {}
            rprintln!("#{} A{} B{} C{}", seq, tag(0), tag(1), tag(2));
        }

        if period >= total_p {
            all_off();
            let _ = writeln!(
                serial,
                "DONE. VERDICT A(PB3){} x{} B(PB7){} x{} C(PA2){} x{} -- each should be LH with crossings.",
                if seen_lo[0] && seen_hi[0] {
                    "=LIVE"
                } else {
                    "=STUCK"
                },
                cross[0],
                if seen_lo[1] && seen_hi[1] {
                    "=LIVE"
                } else {
                    "=STUCK"
                },
                cross[1],
                if seen_lo[2] && seen_hi[2] {
                    "=LIVE"
                } else {
                    "=STUCK"
                },
                cross[2],
            );
            while serial.flush().is_err() {}
            rprintln!("DONE");
            loop {
                cortex_m::asm::nop();
            }
        }
    }
}
