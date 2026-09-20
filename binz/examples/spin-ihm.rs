//! Open-loop sine spin for the X-NUCLEO-IHM08M1 + G071RB rig, six drive signals
//! as PLAIN GPIO (no TIM1) — the "dumb bitbang" first-spin that proves wiring +
//! FETs + driver, adapted from the (retired EVLDRIVE102H) spin-gpio.rs.
//!
//! Soft PWM: 16 kHz slots; each phase runs a sigma-delta on its sine-shaped duty
//! (peak 7% < 10%). Every edge sequences off-side first, ~1 us gap, then on-side
//! (bit-banged dead time) so a phase's HIN/LIN are never both high -> no
//! shoot-through regardless of L6398 interlock.
//!
//! IHM08M1 map: INH1/2/3 = PA8/PA9/PA10 (L6398 HIN); INL1/2/3 = PA7/PB0/PB1
//! (L6398 LIN, active-high). Current = PA0 (shunt amp, mid-rail @ 0 A). Bus =
//! PA1 (x19.15). BKIN = PA6 (OC comparator; polarity auto-detected). NO EN pin
//! on the L6398 — "safe" = all six gates low.
//!
//! Profile: 30 ms low-side bootstrap precharge, 0.5 s park, 5 -> 100 Hz over 3 s,
//! hold to 45 s, stop. Guards: IS overcurrent (2-strike), VM sag, BKIN change.
//! *** Keep the PSU current limit dialed down: it is the hard backstop. ***
//!
//! Run: `cargo run --release --example spin-ihm`

#![no_std]
#![no_main]

use binz as _; // panic handler
use core::fmt::Write;
use cortex_m::asm::delay as cycdelay;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{Adc, Channel, Precision, SampleTime};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::BasicConfig;
use stm32g0xx_hal::stm32;

const SINE_LUT: [u8; 256] = [
    128, 131, 134, 137, 140, 143, 146, 149, 152, 155, 158, 162, 165, 167, 170, 173, 176, 179, 182,
    185, 188, 190, 193, 196, 198, 201, 203, 206, 208, 211, 213, 215, 218, 220, 222, 224, 226, 228,
    230, 232, 234, 235, 237, 238, 240, 241, 243, 244, 245, 246, 248, 249, 250, 250, 251, 252, 253,
    253, 254, 254, 254, 255, 255, 255, 255, 255, 255, 255, 254, 254, 254, 253, 253, 252, 251, 250,
    250, 249, 248, 246, 245, 244, 243, 241, 240, 238, 237, 235, 234, 232, 230, 228, 226, 224, 222,
    220, 218, 215, 213, 211, 208, 206, 203, 201, 198, 196, 193, 190, 188, 185, 182, 179, 176, 173,
    170, 167, 165, 162, 158, 155, 152, 149, 146, 143, 140, 137, 134, 131, 128, 124, 121, 118, 115,
    112, 109, 106, 103, 100, 97, 93, 90, 88, 85, 82, 79, 76, 73, 70, 67, 65, 62, 59, 57, 54, 52,
    49, 47, 44, 42, 40, 37, 35, 33, 31, 29, 27, 25, 23, 21, 20, 18, 17, 15, 14, 12, 11, 10, 9, 7,
    6, 5, 5, 4, 3, 2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 3, 4, 5, 5, 6, 7, 9, 10, 11,
    12, 14, 15, 17, 18, 20, 21, 23, 25, 27, 29, 31, 33, 35, 37, 40, 42, 44, 47, 49, 52, 54, 57, 59,
    62, 65, 67, 70, 73, 76, 79, 82, 85, 88, 90, 93, 97, 100, 103, 106, 109, 112, 115, 118, 121,
    124,
];

const CARRIER_HZ: u32 = 16_000;
const PEAK_DUTY_Q16: u32 = 4587; // 7% of 65536, < 10%
const TARGET_HZ: u32 = 100;
const RAMP_START_HZ: u32 = 5;
const SPIN_SECONDS: u32 = 45;
const IS_KILL_DELTA_MV: u16 = 150; // soft guard (uncalibrated mV/A); PSU is the hard limit
const VM_DIV: u32 = 1915; // bus divider x19.15
const VM_FLOOR_MV: u32 = 6000; // L6398 / FET operating minimum
const DELAY_1US: u32 = 21; // asm::delay ~3 cyc/count @ 64 MHz (same G071)
const PERIOD_UNITS: u32 = 62 * DELAY_1US;
const MIN_ON_UNITS: u32 = DELAY_1US;
const OVERHEAD_UNITS: u32 = 300;

// Current on PA0 = ADC IN0 (was PB11/IN15 on the old board — now BEMF2).
struct ChIs;
impl Channel<Adc> for ChIs {
    type ID = u8;
    fn channel() -> u8 {
        0
    }
}
struct ChBus;
impl Channel<Adc> for ChBus {
    type ID = u8;
    fn channel() -> u8 {
        1
    }
}

// BSRR bits — CORRECTED to the REAL G0 morpho pinout (um2324 Fig 17), not the
// IHM08M1's assumed labels. Highs PA8/9/10 (CN10-23/21/33). Lows: U=PA7
// (CN10-15), V=PB1 (CN7-34, NOT PB0), W=PB6 (CN10-24, NOT PB1).
const INH_A_BIT: [u32; 3] = [8, 9, 10];
const INL_A_BIT: u32 = 7; // phase 0 (U) low = PA7
const INL_B_BIT: [u32; 2] = [1, 6]; // phase 1 (V) = PB1, phase 2 (W) = PB6

/// Stage safe: all six gates low (no EN on the L6398 — this IS the off state).
fn kill(reason: &str) -> ! {
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let pb = &*stm32::GPIOB::ptr();
        pa.bsrr().write(|w| {
            w.bits((1 << (7 + 16)) | (1 << (8 + 16)) | (1 << (9 + 16)) | (1 << (10 + 16)))
        });
        pb.bsrr()
            .write(|w| w.bits((1 << (0 + 16)) | (1 << (1 + 16))));
    }
    rprintln!("!! KILL: {}", reason);
    loop {
        cortex_m::asm::nop();
    }
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

    // Telemetry over the relocated VCOM (USART3/PC10) — read via the serial MCP.
    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();

    let _bus = gpioa.pa1.into_analog();
    let _is = gpioa.pa0.into_analog();
    let mut bkin = gpioa.pa6.into_floating_input();

    // Six plain push-pull outputs, all low.
    let mut inh1 = gpioa.pa8.into_push_pull_output();
    let mut inh2 = gpioa.pa9.into_push_pull_output();
    let mut inh3 = gpioa.pa10.into_push_pull_output();
    let mut inl1 = gpioa.pa7.into_push_pull_output();
    let mut inl2 = gpiob.pb0.into_push_pull_output();
    let mut inl3 = gpiob.pb1.into_push_pull_output();
    inh1.set_low().ok();
    inh2.set_low().ok();
    inh3.set_low().ok();
    inl1.set_low().ok();
    inl2.set_low().ok();
    inl3.set_low().ok();

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();

    // BKIN idle level (OC comparator, unknown polarity) — kill on any change.
    let bkin_idle = bkin.is_high().unwrap();

    let vm0 = adc.read_voltage(&mut ChBus).unwrap() as u32 * VM_DIV / 100;
    rprintln!(
        "spin-ihm: VM~{}.{:02} V, peak 7%, target {} Hz, BKIN idle={}",
        vm0 / 1000,
        (vm0 % 1000) / 10,
        TARGET_HZ,
        bkin_idle as u8
    );
    let _ = writeln!(
        serial,
        "\r\nspin-ihm: VM={}mV peak7% target{}Hz BKINidle={} (need bus>=~12V for L6398 +15V rail)",
        vm0, TARGET_HZ, bkin_idle as u8
    );
    while serial.flush().is_err() {}
    if vm0 < VM_FLOOR_MV {
        let _ = writeln!(serial, "!! VM below floor {}mV -> abort", vm0);
        while serial.flush().is_err() {}
        kill("VM below floor");
    }
    let mut acc32: u32 = 0;
    for _ in 0..16 {
        acc32 += adc.read_voltage(&mut ChIs).unwrap() as u32;
    }
    let is_base = (acc32 / 16) as u16;
    rprintln!("IS baseline {} mV", is_base);

    // Bootstrap precharge: all low sides on 30 ms (pulls OUT low, charges the
    // L6398 BOOT caps through the bootstrap diodes).
    inl1.set_high().ok();
    inl2.set_high().ok();
    inl3.set_high().ok();
    delay.delay(30.millis());
    if bkin.is_high().unwrap() != bkin_idle {
        kill("BKIN after precharge");
    }

    let inc_per_hz: u32 = ((1u64 << 32) / CARRIER_HZ as u64) as u32;
    let mut phase: u32 = 0;
    let mut freq_chz: u32 = RAMP_START_HZ * 100;
    let mut amp_q16: u32 = 0;
    let mut period: u32 = 0;
    let align_p = CARRIER_HZ / 2;
    let ramp_p = CARRIER_HZ * 3;
    let total_p = CARRIER_HZ * SPIN_SECONDS;
    let mut is_now: u16 = is_base;
    let mut oc_strikes: u8 = 0;

    rprintln!(
        "park 0.5 s, ramp {} -> {} Hz over 3 s, hold to {} s",
        RAMP_START_HZ,
        TARGET_HZ,
        SPIN_SECONDS
    );
    loop {
        period += 1;

        if period <= align_p {
            amp_q16 = PEAK_DUTY_Q16 * period / align_p;
        } else {
            phase = phase.wrapping_add((inc_per_hz as u64 * freq_chz as u64 / 100) as u32);
            if period <= align_p + ramp_p {
                let t = period - align_p;
                freq_chz = RAMP_START_HZ * 100 + (TARGET_HZ - RAMP_START_HZ) * 100 * t / ramp_p;
            }
        }

        let mut d = [0u32; 3];
        for k in 0..3usize {
            let idx = (phase.wrapping_add(k as u32 * 0x5555_5555) >> 24) as usize;
            let duty_q16 = amp_q16 * SINE_LUT[idx] as u32 >> 8;
            let units = duty_q16 * PERIOD_UNITS >> 16;
            d[k] = if units >= MIN_ON_UNITS { units } else { 0 };
        }

        let mut ord = [0usize, 1, 2];
        if d[ord[0]] > d[ord[1]] {
            ord.swap(0, 1);
        }
        if d[ord[1]] > d[ord[2]] {
            ord.swap(1, 2);
        }
        if d[ord[0]] > d[ord[1]] {
            ord.swap(0, 1);
        }
        let active_mask_a: u32 = (0..3)
            .filter(|&k| d[k] > 0)
            .map(|k| 1u32 << INH_A_BIT[k])
            .sum();

        if active_mask_a != 0 {
            unsafe {
                let pa = &*stm32::GPIOA::ptr();
                let pb = &*stm32::GPIOB::ptr();
                // INL off for active phases (dead-gap before INH on).
                let mut la: u32 = 0;
                let mut lb: u32 = 0;
                if d[0] > 0 {
                    la |= 1 << (INL_A_BIT + 16);
                }
                if d[1] > 0 {
                    lb |= 1 << (INL_B_BIT[0] + 16);
                }
                if d[2] > 0 {
                    lb |= 1 << (INL_B_BIT[1] + 16);
                }
                if la != 0 {
                    pa.bsrr().write(|w| w.bits(la));
                }
                if lb != 0 {
                    pb.bsrr().write(|w| w.bits(lb));
                }
                cycdelay(DELAY_1US);
                // All active INH on together.
                pa.bsrr().write(|w| w.bits(active_mask_a));
                // Staggered INH offs at each phase's on-time.
                let mut elapsed: u32 = 0;
                for &k in ord.iter() {
                    if d[k] == 0 {
                        continue;
                    }
                    if d[k] > elapsed {
                        cycdelay(d[k] - elapsed);
                        elapsed = d[k];
                    }
                    pa.bsrr().write(|w| w.bits(1 << (INH_A_BIT[k] + 16)));
                }
                cycdelay(DELAY_1US);
                // INLs back on (diode-mode freewheel between INH-off and here).
                let mut la: u32 = 0;
                let mut lb: u32 = 0;
                if d[0] > 0 {
                    la |= 1 << INL_A_BIT;
                }
                if d[1] > 0 {
                    lb |= 1 << INL_B_BIT[0];
                }
                if d[2] > 0 {
                    lb |= 1 << INL_B_BIT[1];
                }
                if la != 0 {
                    pa.bsrr().write(|w| w.bits(la));
                }
                if lb != 0 {
                    pb.bsrr().write(|w| w.bits(lb));
                }
                cycdelay(PERIOD_UNITS.saturating_sub(elapsed + OVERHEAD_UNITS));
            }
        } else {
            cycdelay(PERIOD_UNITS.saturating_sub(OVERHEAD_UNITS));
        }

        // ~1 kHz guards.
        if period % 16 == 0 {
            is_now = adc.read_voltage(&mut ChIs).unwrap();
            if is_now.abs_diff(is_base) > IS_KILL_DELTA_MV {
                oc_strikes += 1;
                if oc_strikes >= 2 {
                    kill("overcurrent (2 consecutive)");
                }
            } else {
                oc_strikes = 0;
            }
            if bkin.is_high().unwrap() != bkin_idle {
                kill("BKIN (overcurrent comparator)");
            }
        }
        if period % (CARRIER_HZ / 2) == 0 {
            let vm = adc.read_voltage(&mut ChBus).unwrap() as u32 * VM_DIV / 100;
            if vm < VM_FLOOR_MV {
                kill("VM sag");
            }
            let _ = writeln!(
                serial,
                "t={}s f={}.{:02}Hz amp={}% dIS={}mV(base{}) VM={}mV",
                period / CARRIER_HZ,
                freq_chz / 100,
                freq_chz % 100,
                amp_q16 * 100 / 65536,
                is_now.abs_diff(is_base),
                is_base,
                vm
            );
            while serial.flush().is_err() {}
        }

        if period >= total_p {
            kill("DONE - 45 s spin complete (stage safed)");
        }
    }
}
