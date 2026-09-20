//! TIM1 hardware-PWM sine spin -- raw PAC register writes only (the HAL
//! PWM layer is scarred: see CLAUDE.md #1; wiring itself is proven good
//! by spin-gpio, which stays the reference if anything looks off).
//!
//! Stage A: 1 kHz carrier (park 0.5 s, ramp 5->100 Hz, hold) to 12 s.
//! Stage B: live-switch to 10 kHz, continue holding 100 Hz to 45 s.
//! Peak duty 7%. NOTE: at 1 kHz a 7% pulse = 70 us of full bus into a
//! low-L winding -- the IS guard may legitimately end stage A early;
//! that is the guard working, not a wiring fault.
//!
//! Monitors, all -> immediate stage disable (MOE off, CCRs 0, EN low):
//!   a) bus voltage: instant kill < 6 V, 2-strike kill < 10.0 V (PSU is
//!      current-limited ~0.8 A; hitting CC mode collapses VM),
//!   b) RSP/RSN shunt current via the STDRIVE amp (IS, ~60 mV/A around
//!      1.65 V): 2-strike kill at |delta| > 150 mV (~2.5 A),
//!   c) nFLT (shared EN node): any low -> kill.
//! Plus NTC > 60 C and a pre-spin 50%-duty self-test that proves the
//! high sides really switch (VPH must read ~378 mV) before the motor
//! sees any waveform.
//!
//! Run: `cargo run --release --example spin-pwm`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{Adc, Channel, Precision, SampleTime};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
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

const SYSCLK: u32 = 64_000_000;
const STAGE_A_HZ: u32 = 1_000;
const STAGE_B_HZ: u32 = 10_000;
// 1 kHz trial result (2026-09-05): RSP/RSN guard trips ~100 ms into the
// ramp -- 70 us full-bus pulses are multi-amp spikes in this low-L motor.
// Stage A therefore skipped by default; set >0 to re-try a slow carrier.
const STAGE_A_SECONDS: u32 = 0;
const SPIN_SECONDS: u32 = 45;
const PEAK_DUTY_PCT: u32 = 7; // < 8% cap
const TARGET_HZ: u32 = 100;
const RAMP_START_HZ: u32 = 5;
const DTG: u8 = 26; // ~406 ns
const IS_KILL_DELTA_MV: u16 = 150; // ~2.5 A on RSP/RSN via G=12 amp
const VM_SAG_MV: u32 = 10_000; // risky-sag threshold (2-strike)
const VM_DEAD_MV: u32 = 6_000; // instant

struct ChIs;
impl Channel<Adc> for ChIs {
    type ID = u8;
    fn channel() -> u8 {
        15 // PB11 = shunt amp out (RSP/RSN)
    }
}
struct ChBus;
impl Channel<Adc> for ChBus {
    type ID = u8;
    fn channel() -> u8 {
        1 // PA1
    }
}
struct ChTemp;
impl Channel<Adc> for ChTemp {
    type ID = u8;
    fn channel() -> u8 {
        17 // PC4
    }
}
struct ChVph1;
impl Channel<Adc> for ChVph1 {
    type ID = u8;
    fn channel() -> u8 {
        9 // PB1
    }
}

/// Stage safe from anywhere: outputs idle-low, CCRs 0, EN low.
fn kill(reason: &str) -> ! {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.bdtr().modify(|r, w| w.bits(r.bits() & !(1 << 15))); // MOE off -> OSSI idle low
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        let pc = &*stm32::GPIOC::ptr();
        pc.bsrr().write(|w| w.br9().set_bit()); // EN low (red LED = safed)
    }
    rprintln!("!! KILL: {}", reason);
    loop {
        cortex_m::asm::nop();
    }
}

/// Program the carrier: PSC=0, ARR for `hz`, force-load via UG.
/// Returns ARR. Call with CCRs at 0.
fn set_carrier(hz: u32) -> u32 {
    let arr = SYSCLK / hz - 1;
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.arr().write(|w| w.bits(arr));
        tim.egr().write(|w| w.bits(1)); // UG: latch ARR/CCR/PSC now
    }
    arr
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
    let gpiod = dp.GPIOD.split(&mut rcc);

    let _bus = gpioa.pa1.into_analog();
    let _is = gpiob.pb11.into_analog();
    let _temp = gpioc.pc4.into_analog();
    let _vph1 = gpiob.pb1.into_analog();
    let mut nflt = gpioa.pa6.into_floating_input();

    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok();

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();

    // GPIO AF2 for all six TIM1 pins (this exact config carried the
    // working low sides before; PA8-10 verified by register readback).
    let _ = (
        gpioa.pa7, gpioa.pa8, gpioa.pa9, gpioa.pa10, gpiod.pd3, gpiod.pd4,
    );
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        pa.moder().modify(|_, w| {
            w.moder7()
                .alternate()
                .moder8()
                .alternate()
                .moder9()
                .alternate()
                .moder10()
                .alternate()
        });
        pa.afrl().modify(|_, w| w.afr(7).af2());
        // AFRH indexes 0-7 for pins 8-15.
        pa.afrh()
            .modify(|_, w| w.afr(0).af2().afr(1).af2().afr(2).af2());
        let pd = &*stm32::GPIOD::ptr();
        pd.moder()
            .modify(|_, w| w.moder3().alternate().moder4().alternate());
        pd.afrl().modify(|_, w| w.afr(3).af2().afr(4).af2());
    }

    // TIM1: raw register values straight from RM0444, no HAL, no field
    // roulette. PWM1+preload all 3 channels, all six outputs enabled,
    // dead time, OSSI/OSSR (idle = driven low), ARPE.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 11))); // TIM1EN
        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0)); // stopped while configuring
        tim.cr2().write(|w| w.bits(0));
        tim.psc().write(|w| w.bits(0));
        tim.rcr().write(|w| w.bits(0));
        tim.ccmr1_output().write(|w| w.bits(0x6868)); // OC1/OC2: PWM1 + PE
        tim.ccmr2_output().write(|w| w.bits(0x0068)); // OC3: PWM1 + PE
        tim.ccer().write(|w| w.bits(0x0555)); // CCxE + CCxNE, active high
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        tim.bdtr()
            .write(|w| w.bits((1 << 11) | (1 << 10) | DTG as u32)); // OSSR|OSSI|DTG, MOE off
    }
    let mut arr = set_carrier(STAGE_A_HZ);
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0x81)); // ARPE | CEN
    }

    let vm0 = adc.read_voltage(&mut ChBus).unwrap() as u32 * 1759 / 100;
    rprintln!(
        "spin-pwm: stage A {} Hz (ARR={}), stage B {} Hz at t={}s, peak duty {}%, VM~{}.{:02} V",
        STAGE_A_HZ,
        arr,
        STAGE_B_HZ,
        STAGE_A_SECONDS,
        PEAK_DUTY_PCT,
        vm0 / 1000,
        (vm0 % 1000) / 10
    );
    if vm0 < VM_DEAD_MV {
        kill("VM below 6 V");
    }
    let mut acc: u32 = 0;
    for _ in 0..16 {
        acc += adc.read_voltage(&mut ChIs).unwrap() as u32;
    }
    let is_base = (acc / 16) as u16;
    rprintln!("IS baseline {} mV (RSP/RSN amp)", is_base);

    // Enable driver + precharge (CCR=0 -> low sides on via OCxN).
    en.set_high().ok();
    delay.delay(2.millis());
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.bdtr().modify(|r, w| w.bits(r.bits() | (1 << 15))); // MOE
    }
    delay.delay(30.millis());
    if nflt.is_low().unwrap() {
        kill("nFLT after enable");
    }

    // SELF-TEST: all phases 50% for 20 ms (zero line-line volts, zero
    // motor current). High sides MUST switch: VPH1 ~ VM/2 / 15.67.
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        let half = (arr + 1) / 2;
        tim.ccr1().write(|w| w.bits(half));
        tim.ccr2().write(|w| w.bits(half));
        tim.ccr3().write(|w| w.bits(half));
    }
    delay.delay(20.millis());
    // Average 32 samples spread over many carrier periods (a single ADC
    // sample is ~5 us -- instantaneous, not an average). 50% duty with
    // working complementary outputs -> ~378 mV; high-only (low side
    // dead) -> ~640 mV; never-high -> ~0-20 mV.
    let mut vsum: u32 = 0;
    for _ in 0..32 {
        vsum += adc.read_voltage(&mut ChVph1).unwrap() as u32;
        delay.delay(337.micros());
    }
    let vph_test = (vsum / 32) as u16;
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
    }
    rprintln!("self-test vph1@50% avg = {} mV (want ~378)", vph_test);
    if vph_test < 250 || vph_test > 520 {
        kill("TIM1 outputs NOT switching correctly (CLAUDE.md scar #1) - fall back to spin-gpio");
    }
    delay.delay(10.millis());

    // Spin profile, paced on TIM1 update events.
    let mut carrier = STAGE_A_HZ;
    let mut amp_peak: u32 = (arr + 1) * PEAK_DUTY_PCT / 100;
    let mut phase: u32 = 0;
    let mut freq_chz: u32 = RAMP_START_HZ * 100;
    let mut amp: u32 = 0;
    let mut t_ms: u32 = 0; // wall time in ms, carrier-independent
    let mut period_in_ms: u32 = 0;
    let mut is_now: u16 = is_base;
    let mut oc_strikes: u8 = 0;
    let mut sag_strikes: u8 = 0;
    let mut guard_phase: bool = false;
    let tim = unsafe { &*stm32::TIM1::ptr() };

    rprintln!(
        "park 0.5 s, ramp {} -> {} Hz over 3 s",
        RAMP_START_HZ,
        TARGET_HZ
    );
    loop {
        while tim.sr().read().bits() & 1 == 0 {}
        tim.sr().modify(|r, w| unsafe { w.bits(r.bits() & !1) });
        period_in_ms += 1;
        if period_in_ms >= carrier / 1000 {
            period_in_ms = 0;
            t_ms += 1;
        }

        // Profile: park to 500 ms, ramp to 3500 ms, hold after.
        if t_ms <= 500 {
            amp = amp_peak * t_ms / 500;
        } else {
            phase = phase
                .wrapping_add(((freq_chz as u64) * (1u64 << 32) / (carrier as u64 * 100)) as u32);
            if t_ms <= 3500 {
                freq_chz =
                    RAMP_START_HZ * 100 + (TARGET_HZ - RAMP_START_HZ) * 100 * (t_ms - 500) / 3000;
            }
        }

        let i0 = (phase >> 24) as usize;
        let i1 = (phase.wrapping_add(0x5555_5555) >> 24) as usize;
        let i2 = (phase.wrapping_add(0xAAAA_AAAA) >> 24) as usize;
        unsafe {
            tim.ccr1().write(|w| w.bits(amp * SINE_LUT[i0] as u32 >> 8));
            tim.ccr2().write(|w| w.bits(amp * SINE_LUT[i1] as u32 >> 8));
            tim.ccr3().write(|w| w.bits(amp * SINE_LUT[i2] as u32 >> 8));
        }

        // Guards every ms (alternating IS / BUS so each ADC read fits).
        if period_in_ms == 0 {
            guard_phase = !guard_phase;
            if guard_phase {
                is_now = adc.read_voltage(&mut ChIs).unwrap();
                if is_now.abs_diff(is_base) > IS_KILL_DELTA_MV {
                    oc_strikes += 1;
                    if oc_strikes >= 2 {
                        kill("overcurrent (RSP/RSN)");
                    }
                } else {
                    oc_strikes = 0;
                }
            } else {
                let vm = adc.read_voltage(&mut ChBus).unwrap() as u32 * 1759 / 100;
                if vm < VM_DEAD_MV {
                    kill("VM collapsed");
                }
                if vm < VM_SAG_MV {
                    sag_strikes += 1;
                    if sag_strikes >= 2 {
                        kill("VM sag (PSU current limit?)");
                    }
                } else {
                    sag_strikes = 0;
                }
            }
            if nflt.is_low().unwrap() {
                kill("nFLT");
            }
        }

        // Stage B switch at STAGE_A_SECONDS: zero duty, retime, resume.
        if carrier == STAGE_A_HZ && t_ms >= STAGE_A_SECONDS * 1000 {
            unsafe {
                tim.ccr1().write(|w| w.bits(0));
                tim.ccr2().write(|w| w.bits(0));
                tim.ccr3().write(|w| w.bits(0));
            }
            arr = set_carrier(STAGE_B_HZ);
            carrier = STAGE_B_HZ;
            amp_peak = (arr + 1) * PEAK_DUTY_PCT / 100;
            rprintln!("== stage B: carrier {} Hz (ARR={}) ==", STAGE_B_HZ, arr);
        }

        // Telemetry every 500 ms.
        if period_in_ms == 0 && t_ms % 500 == 0 {
            // NTC voltage RISES with temperature (NTC on top of divider);
            // ~993 mV at 60 C with B=3435.
            let temp = adc.read_voltage(&mut ChTemp).unwrap();
            if temp > 900 {
                kill("power stage over ~60 C");
            }
            let vph = adc.read_voltage(&mut ChVph1).unwrap();
            let ma = is_now.abs_diff(is_base) as u32 * 100 / 6;
            rprintln!(
                "t={}ms carrier={} f={}.{:02}Hz amp={}/{} vph1={}mV |I|~{}mA",
                t_ms,
                carrier,
                freq_chz / 100,
                freq_chz % 100,
                amp,
                arr + 1,
                vph,
                ma
            );
        }

        if t_ms >= SPIN_SECONDS * 1000 {
            kill("DONE - 45 s complete (stage safed; re-run to spin again)");
        }
    }
}
