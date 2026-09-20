//! Open-loop six-step spin for the BOOSTXL-DRV8304H in **3x PWM mode** (its
//! power-up default: R19 47k->AGND straps MODE to 3x). First "does it turn"
//! smoke test after the 6 gate wires were landed. NO sense pins wired yet
//! (no VBUS/current/nFAULT), so the ONLY protection is the bench PSU current
//! limit -- keep it LOW (~0.5 A) and the bus low.
//!
//! 3x mode truth table (DRV8304 datasheet Table 3): INLx = ENABLE (0 -> phase
//! Hi-Z), INHx = STATE (1 -> high-side, 0 -> low-side); the driver makes the
//! complementary pair + dead-time internally. So per commutation step:
//!   source phase: INL=1, INH=PWM (duty)
//!   sink   phase: INL=1, INH=0   (low-side on)
//!   float  phase: INL=0          (Hi-Z, for future BEMF)
//! A 6x-style pattern (INL dropped on the active high phase) would leave that
//! phase Hi-Z and NOT spin -- that's why this is 3x-native.
//!
//! Pins (DRV wire map, verified): INH A/B/C = PA10/PA9/PA8; INL A/B/C =
//! PB1/PB0/PA7. Status over USART3 (PC10) @115200 -- watch on the FTDI VCOM.
//! ENABLE (J5-9) assumed tied high; STBY n/a on this board.
//!
//! Profile: park 0.3 s, ramp 5->30 eHz over 4 s, hold 6 s, stop. ~16 kHz PWM,
//! V/f duty 3..8 %. Run: cargo run --release --example drv-spin3x

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

const DELAY_1US: u32 = 21; // asm::delay ~3 cyc/count @64 MHz (spin-gpio calib)
const CARRIER_HZ: u32 = 16_000;
const PERIOD_UNITS: u32 = 62 * DELAY_1US; // ~62 us PWM period
const OVERHEAD_UNITS: u32 = 60; // per-period branch/BSRR fudge
const MIN_ON_UNITS: u32 = DELAY_1US / 2;

// INH on GPIOA: phase A=PA10, B=PA9, C=PA8.
const INH_BIT: [u32; 3] = [10, 9, 8];
// INL: phase A=GPIOB1, B=GPIOB0, C=GPIOA7. (port: 0=A,1=B ; bit)
const INL_PORT: [u8; 3] = [1, 1, 0]; // 1=GPIOB, 0=GPIOA
const INL_BIT: [u32; 3] = [1, 0, 7];

// Six-step forward: (source, sink); float = the remaining phase.
const STEP: [(usize, usize); 6] = [(0, 1), (0, 2), (1, 2), (1, 0), (2, 0), (2, 1)];

#[inline(always)]
fn bsrr_a(set_mask: u32, clr_mask: u32) {
    unsafe {
        (*stm32::GPIOA::ptr())
            .bsrr()
            .write(|w| w.bits(set_mask | (clr_mask << 16)));
    }
}
#[inline(always)]
fn bsrr_b(set_mask: u32, clr_mask: u32) {
    unsafe {
        (*stm32::GPIOB::ptr())
            .bsrr()
            .write(|w| w.bits(set_mask | (clr_mask << 16)));
    }
}

#[inline(always)]
fn set_inl(phase: usize, high: bool) {
    let bit = 1u32 << INL_BIT[phase];
    let (s, c) = if high { (bit, 0) } else { (0, bit) };
    if INL_PORT[phase] == 0 {
        bsrr_a(s, c);
    } else {
        bsrr_b(s, c);
    }
}

/// Apply INL enables + park sink/float INH low for a step; return source phase.
fn commutate(step: usize) -> usize {
    let (src, sink) = STEP[step];
    let flt = 3 - src - sink;
    // All INH low first (drop the old source cleanly).
    bsrr_a(0, (1 << INH_BIT[0]) | (1 << INH_BIT[1]) | (1 << INH_BIT[2]));
    set_inl(src, true);
    set_inl(sink, true);
    set_inl(flt, false); // Hi-Z floating phase
    src
}

/// Stage-safe: all six gates low (all Hi-Z / off).
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

    // Six gate outputs, all low BEFORE anything can float a FET on.
    let _ = (
        gpioa.pa7.into_push_pull_output(),
        gpioa.pa8.into_push_pull_output(),
        gpioa.pa9.into_push_pull_output(),
        gpioa.pa10.into_push_pull_output(),
        gpiob.pb0.into_push_pull_output(),
        gpiob.pb1.into_push_pull_output(),
    );
    all_off();

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
        "\r\ndrv-spin3x: 3x-mode six-step, INL=enable. NO current sense -> PSU limit is your guard."
    );
    while serial.flush().is_err() {}
    rprintln!("drv-spin3x start");

    // Park: enable step 0's phases at 0 duty for 0.3 s (settle, charge pump).
    let mut src = commutate(0);
    delay.delay(300.millis());

    let ramp_p = CARRIER_HZ * 4; // 4 s ramp
    let total_p = CARRIER_HZ * 10; // 10 s total
    let mut period: u32 = 0;
    let mut step: usize = 0;
    let mut since_comm: u32 = 0;

    loop {
        period += 1;

        // eHz: 5 -> 30 over the ramp, then hold 30.
        let ehz = if period <= ramp_p {
            5 + 25 * period / ramp_p
        } else {
            30
        };
        // V/f duty: 3 % + 0.18 %/Hz, capped 8 %.
        let duty_pct = core::cmp::min(300 + 18 * ehz, 800); // percent*100
        let duty_q16 = duty_pct * 65536 / 10000;
        let mut on_units = duty_q16 * PERIOD_UNITS >> 16;
        if on_units < MIN_ON_UNITS {
            on_units = 0;
        }

        // Commutate at 6*eHz.
        let step_len = CARRIER_HZ / (6 * ehz);
        since_comm += 1;
        if since_comm >= step_len {
            since_comm = 0;
            step = (step + 1) % 6;
            src = commutate(step);
        }

        // PWM the source INH for this period.
        let inh = 1u32 << INH_BIT[src];
        if on_units > 0 {
            bsrr_a(inh, 0);
            cycdelay(on_units);
            bsrr_a(0, inh);
        }
        cycdelay(PERIOD_UNITS.saturating_sub(on_units + OVERHEAD_UNITS));

        // Status ~2 Hz (brief blocking write; tiny PWM hiccup is fine open-loop).
        if period % (CARRIER_HZ / 2) == 0 {
            let _ = writeln!(
                serial,
                "t={}s eHz={} duty={}.{:02}% step={}",
                period / CARRIER_HZ,
                ehz,
                duty_pct / 100,
                duty_pct % 100,
                step
            );
            while serial.flush().is_err() {}
            rprintln!("t={}s eHz={} step={}", period / CARRIER_HZ, ehz, step);
        }

        if period >= total_p {
            all_off();
            let _ = writeln!(serial, "DONE - 10 s open-loop spin complete, stage safed.");
            while serial.flush().is_err() {}
            rprintln!("DONE");
            loop {
                cortex_m::asm::nop();
            }
        }
    }
}
