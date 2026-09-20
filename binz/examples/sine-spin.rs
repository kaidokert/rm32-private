//! Open-loop sinusoidal spin on the EVLDRIVE102H (G071 bridge config).
//!
//! TIM1 @ 24 kHz center of the drive: CH1/2/3 = INH on PA8/PA9/PA10 (HAL
//! pwm+bind_pin, AF2), CH1N/2N/3N = INL on PA7/PD3/PD4 (PAC AF2 config,
//! HAL doesn't expose complementary pins), hardware dead-time ~400 ns.
//! Three 120-degree-spaced sine duties from a 256-entry LUT, phase
//! accumulator stepped every PWM update.
//!
//! Envelope: peak duty capped at 7% (< 8% requirement), electrical
//! frequency ramped 5 -> 100 Hz over ~3 s after a park/align, then held.
//!
//! Guards (bench rules): IS shunt-amp delta kill at ~2.5 A, nFLT kill,
//! power-stage NTC kill at 60 C, auto-stop after 45 s of spin. Every kill
//! path forces MOE=0 (OSSI/OSSR -> gates low) and EN low.
//!
//! Run: `cargo run --release --example sine-spin`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{Adc, Channel, Precision, SampleTime};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::timer::pwm::PwmExt;

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

const PWM_HZ: u32 = 24_000;
const PEAK_DUTY_PCT: u32 = 7; // < 8% requirement
const TARGET_HZ: u32 = 100;
const RAMP_START_HZ: u32 = 5;
const DTG: u8 = 26; // ~406 ns dead time @ 64 MHz
const IS_KILL_DELTA_MV: u16 = 150; // ~2.5 A at G=12, 5 mOhm shunt
const TEMP_KILL_C: i32 = 60;
const SPIN_SECONDS: u32 = 45;

struct ChIs;
impl Channel<Adc> for ChIs {
    type ID = u8;
    fn channel() -> u8 {
        15 // PB11
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
struct ChVph2;
impl Channel<Adc> for ChVph2 {
    type ID = u8;
    fn channel() -> u8 {
        8 // PB0
    }
}
struct ChVph3;
impl Channel<Adc> for ChVph3 {
    type ID = u8;
    fn channel() -> u8 {
        10 // PB2
    }
}

fn ln_approx(x: f32) -> f32 {
    let y = (x - 1.0) / (x + 1.0);
    let y2 = y * y;
    2.0 * y * (1.0 + y2 / 3.0 + y2 * y2 / 5.0 + y2 * y2 * y2 / 7.0)
}

fn ntc_c(temp_mv: u16) -> i32 {
    if temp_mv < 50 {
        return i32::MIN;
    }
    let r_ntc = 1.291 * (3300.0 / temp_mv as f32 - 1.0);
    (3435.0 / (ln_approx(r_ntc / 10.0) + 3435.0 / 298.15) - 273.15) as i32
}

/// Force the power stage safe: gates low, driver disabled. PAC-only so it
/// can fire from anywhere regardless of pin ownership.
fn kill(reason: &str) -> ! {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.bdtr().modify(|_, w| w.moe().clear_bit()); // OSSI: outputs -> low
        let pc = &*stm32::GPIOC::ptr();
        pc.bsrr().write(|w| w.br9().set_bit()); // EN (PC9) low
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
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);

    // Sense pins
    let _bus = gpioa.pa1.into_analog();
    let _is = gpiob.pb11.into_analog();
    let _temp = gpioc.pc4.into_analog();
    let _vph1 = gpiob.pb1.into_analog();
    let _vph2 = gpiob.pb0.into_analog();
    let _vph3 = gpiob.pb2.into_analog();
    let mut nflt = gpioa.pa6.into_floating_input();

    // Driver control. CRITICAL: JP1 (default 1-2) ties EN and nFAULT into
    // one node with a 33k pull-up + the red LED. EN must be OPEN-DRAIN:
    // low = force-disable, released = enabled via pull-up, and the driver
    // can still yank the node low to signal/auto-release faults. A
    // push-pull high here masks every fault and blocks latch release.
    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok(); // out of standby

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();

    // TIM1 24 kHz, CH1-3 on PA8/PA9/PA10 (AF2 via HAL).
    let pwm = dp.TIM1.pwm(PWM_HZ.Hz(), &mut rcc);
    let mut ch1 = pwm.bind_pin(gpioa.pa8);
    let mut ch2 = pwm.bind_pin(gpioa.pa9);
    let mut ch3 = pwm.bind_pin(gpioa.pa10);
    let max_duty: u32 = ch1.get_max_duty() as u32; // = ARR

    // Complementary pins: PA7=CH1N, PD3=CH2N, PD4=CH3N, all AF2.
    // (HAL keeps set_alt_mode crate-private, so PAC it is. gpiod.split()
    // above enabled the GPIOD clock; the pin tokens stay unused.)
    let _ = (gpiod.pd3, gpiod.pd4, gpioa.pa7);
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        pa.moder().modify(|_, w| w.moder7().alternate());
        pa.afrl().modify(|_, w| w.afr(7).af2());
        let pd = &*stm32::GPIOD::ptr();
        pd.moder()
            .modify(|_, w| w.moder3().alternate().moder4().alternate());
        pd.afrl().modify(|_, w| w.afr(3).af2().afr(4).af2());
    }

    // Advanced-timer extras: complementary enables, dead time, safe-off
    // states (OSSI/OSSR with OIS=0 => pins driven LOW whenever MOE=0).
    let tim = unsafe { &*stm32::TIM1::ptr() };
    // HAL's bind_pin for TIM1 sets CCxNE as its enable (macro substitutes
    // cc1ne for cc1e), leaving the main outputs OFF -- found live as
    // CCER=0x444 with dead high-sides. Set BOTH so OCx=INH / OCxN=INL run
    // truly complementary.
    tim.ccer().modify(|_, w| {
        w.cc1e()
            .set_bit()
            .cc2e()
            .set_bit()
            .cc3e()
            .set_bit()
            .cc1ne()
            .set_bit()
            .cc2ne()
            .set_bit()
            .cc3ne()
            .set_bit()
    });
    tim.bdtr()
        .modify(|_, w| unsafe { w.dtg().bits(DTG).ossi().set_bit().ossr().set_bit() });
    ch1.set_duty(0);
    ch2.set_duty(0);
    ch3.set_duty(0);

    let vm0 = adc.read_voltage(&mut ChBus).unwrap() as u32 * 1759 / 100;
    rprintln!(
        "sine-spin: ARR={} peak_duty={}% ({} counts) VM~{}.{:02} V",
        max_duty,
        PEAK_DUTY_PCT,
        max_duty * PEAK_DUTY_PCT / 100,
        vm0 / 1000,
        (vm0 % 1000) / 10
    );
    if vm0 < 6000 {
        kill("VM below 6 V - power the bus first");
    }

    // IS baseline with driver disabled (shunt amp mid-rail bias).
    let mut acc: u32 = 0;
    for _ in 0..16 {
        acc += adc.read_voltage(&mut ChIs).unwrap() as u32;
    }
    let is_base = (acc / 16) as u16;
    rprintln!(
        "IS baseline = {} mV; kill at +/-{} mV",
        is_base,
        IS_KILL_DELTA_MV
    );

    // Enable driver, precharge bootstraps: 30 ms of low-sides on (duty 0).
    en.set_high().ok();
    delay.delay(1.millis());
    tim.bdtr().modify(|_, w| w.moe().set_bit());
    delay.delay(30.millis());
    if nflt.is_low().unwrap() {
        kill("nFLT asserted right after enable");
    }

    // --- one-shot diagnostic dump: prove the stage is actually switching ---
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let pd = &*stm32::GPIOD::ptr();
        let pc = &*stm32::GPIOC::ptr();
        let odr = pc.odr().read().bits();
        rprintln!(
            "DIAG en(PC9)={} stby(PC8)={} nflt_hi={}",
            (odr >> 9) & 1,
            (odr >> 8) & 1,
            nflt.is_high().unwrap()
        );
        rprintln!(
            "DIAG tim1: cr1={:#x} ccer={:#x} bdtr={:#x} arr={} psc={} ccr={}/{}/{}",
            tim.cr1().read().bits(),
            tim.ccer().read().bits(),
            tim.bdtr().read().bits(),
            tim.arr().read().bits(),
            tim.psc().read().bits(),
            tim.ccr1().read().bits(),
            tim.ccr2().read().bits(),
            tim.ccr3().read().bits()
        );
        let c1 = tim.cnt().read().bits();
        let c2 = tim.cnt().read().bits();
        rprintln!(
            "DIAG cnt {} -> {} (counting={}) gpioa moder={:#010x} afrh={:#010x} afrl={:#010x} gpiod moder={:#010x} afrl={:#010x}",
            c1,
            c2,
            c1 != c2,
            pa.moder().read().bits(),
            pa.afrh().read().bits(),
            pa.afrl().read().bits(),
            pd.moder().read().bits(),
            pd.afrl().read().bits()
        );
    }
    // VPH with low-sides on (precharge state): should be ~0 mV if the low
    // FETs really conduct; ~518 mV means the stage is NOT switching.
    let vp1 = adc.read_voltage(&mut ChVph1).unwrap();
    let vp2 = adc.read_voltage(&mut ChVph2).unwrap();
    let vp3 = adc.read_voltage(&mut ChVph3).unwrap();
    rprintln!(
        "DIAG vph(low-sides-on) = {}/{}/{} mV (expect ~0)",
        vp1,
        vp2,
        vp3
    );

    // DIAG2: all phases to 50% duty -- identical duty means zero line-line
    // voltage and zero motor current, but VPH must read ~VM/2/5.31
    // (~1100 mV) if the HIGH-side path (CCR -> OC -> INH -> bootstrap ->
    // gate) works.
    let half = (max_duty / 2) as u16;
    ch1.set_duty(half);
    ch2.set_duty(half);
    ch3.set_duty(half);
    delay.delay(20.millis());
    let vp1 = adc.read_voltage(&mut ChVph1).unwrap();
    let vp2 = adc.read_voltage(&mut ChVph2).unwrap();
    let vp3 = adc.read_voltage(&mut ChVph3).unwrap();
    unsafe {
        let t = &*stm32::TIM1::ptr();
        rprintln!(
            "DIAG2 vph(all-50%) = {}/{}/{} mV (expect ~1100) ccr={}/{}/{} cnt={} nflt_hi={}",
            vp1,
            vp2,
            vp3,
            t.ccr1().read().bits(),
            t.ccr2().read().bits(),
            t.ccr3().read().bits(),
            t.cnt().read().bits(),
            nflt.is_high().unwrap()
        );
    }
    // DIAG3 mode discriminator: force INL pins LOW as GPIO, keep INH at
    // 50%. Direct mode: high FETs fire 50% -> VPH ~1100 mV. EN/IN mode
    // (INL=ENx=0): everything off, phases float -> VPH ~400-600 mV.
    // Zero motor current either way (all phases identical).
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let pd = &*stm32::GPIOD::ptr();
        pa.bsrr().write(|w| w.br7().set_bit());
        pa.moder().modify(|_, w| w.moder7().output());
        pd.bsrr().write(|w| w.br3().set_bit().br4().set_bit());
        pd.moder()
            .modify(|_, w| w.moder3().output().moder4().output());
    }
    delay.delay(20.millis());
    let vp1 = adc.read_voltage(&mut ChVph1).unwrap();
    let vp2 = adc.read_voltage(&mut ChVph2).unwrap();
    let vp3 = adc.read_voltage(&mut ChVph3).unwrap();
    let enin_mode = vp1 < 800;
    rprintln!(
        "DIAG3 vph(INL=0, INH=50%) = {}/{}/{} mV nflt_hi={} -> tentative mode: {}",
        vp1,
        vp2,
        vp3,
        nflt.is_high().unwrap(),
        if enin_mode {
            "EN/IN (3-wire)"
        } else {
            "direct INH/INL"
        }
    );
    ch1.set_duty(0);
    ch2.set_duty(0);
    ch3.set_duty(0);

    // DIAG4 fault-catcher: release any latched VDS fault (EN low > release
    // pulse), then command ONE high-side on (phase 1 only, others floating
    // -> no current path) and micro-poll nFLT to timestamp a trip.
    en.set_low().ok();
    delay.delay(20.millis());
    en.set_high().ok();
    delay.delay(5.millis());
    rprintln!(
        "DIAG4 after EN release: nflt_hi={}",
        nflt.is_high().unwrap()
    );
    ch1.set_duty(max_duty as u16); // phase1 high-side ON (INL1 stays GPIO low)
    let mut trip_us: i32 = -1;
    let mut low_samples: u32 = 0;
    for i in 0..1000u32 {
        // ~1 us per iter; EN is open-drain now so the shared EN/nFLT node
        // shows real driver faults (dips = fault + auto-release retry).
        if nflt.is_low().unwrap() {
            low_samples += 1;
            if trip_us < 0 {
                trip_us = i as i32;
            }
        }
        cortex_m::asm::delay(64); // ~1 us
    }
    let vp1 = adc.read_voltage(&mut ChVph1).unwrap();
    ch1.set_duty(0);
    rprintln!(
        "DIAG4 high-side-only pulse: first_fault_us={} fault_low_samples={}/1000 vph1={} mV (~2230 = high FET works)",
        trip_us,
        low_samples,
        vp1
    );

    if enin_mode {
        // EN/IN drive: INL pins stay GPIO but HIGH (ENx=1, phases armed);
        // INH carries the sine duty, driver does complementary + dead time
        // internally.
        unsafe {
            let pa = &*stm32::GPIOA::ptr();
            let pd = &*stm32::GPIOD::ptr();
            pa.bsrr().write(|w| w.bs7().set_bit());
            pd.bsrr().write(|w| w.bs3().set_bit().bs4().set_bit());
        }
    } else {
        // Direct mode: back to complementary AF outputs.
        unsafe {
            let pa = &*stm32::GPIOA::ptr();
            let pd = &*stm32::GPIOD::ptr();
            pa.moder().modify(|_, w| w.moder7().alternate());
            pd.moder()
                .modify(|_, w| w.moder3().alternate().moder4().alternate());
        }
    }
    delay.delay(5.millis());

    let amp_full: u32 = max_duty * PEAK_DUTY_PCT / 100; // peak CCR
    let upd_hz: u32 = PWM_HZ; // one update per PWM period
    let inc_per_hz: u32 = ((1u64 << 32) / upd_hz as u64) as u32;

    let mut phase: u32 = 0;
    let mut freq_chz: u32 = RAMP_START_HZ * 100; // centi-Hz
    let mut amp: u32 = 0;
    let mut tick: u32 = 0;
    let align_ticks = upd_hz / 2; // 0.5 s park/align
    let ramp_ticks = upd_hz * 3; // 3 s frequency ramp
    let total_ticks = upd_hz * SPIN_SECONDS;
    let mut is_now: u16 = is_base;
    let mut temp_now: i32 = 0;
    let mut vm_now: u32 = vm0;

    rprintln!(
        "park/align 0.5 s, ramp {} -> {} Hz over 3 s, hold...",
        RAMP_START_HZ,
        TARGET_HZ
    );
    loop {
        // Pace on TIM1 update (24 kHz).
        while tim.sr().read().uif().bit_is_clear() {}
        tim.sr().modify(|_, w| w.uif().clear_bit());
        tick += 1;

        if tick <= align_ticks {
            // Park: fixed phase, amplitude ramps 0 -> full.
            amp = amp_full * tick / align_ticks;
        } else if tick <= align_ticks + ramp_ticks {
            let t = tick - align_ticks;
            freq_chz = RAMP_START_HZ * 100 + (TARGET_HZ - RAMP_START_HZ) * 100 * t / ramp_ticks;
            phase = phase.wrapping_add((inc_per_hz as u64 * freq_chz as u64 / 100) as u32);
        } else {
            phase = phase.wrapping_add((inc_per_hz as u64 * freq_chz as u64 / 100) as u32);
        }

        let i0 = (phase >> 24) as usize;
        let i1 = (phase.wrapping_add(0x5555_5555) >> 24) as usize;
        let i2 = (phase.wrapping_add(0xAAAA_AAAA) >> 24) as usize;
        ch1.set_duty(((amp * SINE_LUT[i0] as u32) >> 8) as u16);
        ch2.set_duty(((amp * SINE_LUT[i1] as u32) >> 8) as u16);
        ch3.set_duty(((amp * SINE_LUT[i2] as u32) >> 8) as u16);

        // 1 kHz guard cadence (ADC read ~fits one PWM period; an
        // occasionally missed update only nudges the frequency).
        if tick % 24 == 0 {
            is_now = adc.read_voltage(&mut ChIs).unwrap();
            let delta = is_now.abs_diff(is_base);
            if delta > IS_KILL_DELTA_MV {
                kill("overcurrent (IS delta)");
            }
            if nflt.is_low().unwrap() {
                kill("nFLT asserted");
            }
        }
        // Slow telemetry every 500 ms.
        if tick % (upd_hz / 2) == 0 {
            temp_now = ntc_c(adc.read_voltage(&mut ChTemp).unwrap());
            vm_now = adc.read_voltage(&mut ChBus).unwrap() as u32 * 1759 / 100;
            if temp_now > TEMP_KILL_C {
                kill("power stage over 60 C");
            }
            if vm_now < 6000 {
                kill("VM sagged below 6 V (supply current limit?)");
            }
            let ma = (is_now.abs_diff(is_base)) as u32 * 100 / 6; // 60 mV/A
            let vp1 = adc.read_voltage(&mut ChVph1).unwrap();
            rprintln!(
                "vph1={} mV | t={}s f={}.{:02}Hz amp={}({}%) |I|~{} mA VM~{}.{:02}V temp={}C nFLT=ok",
                vp1,
                tick / upd_hz,
                freq_chz / 100,
                freq_chz % 100,
                amp,
                amp * 100 / max_duty,
                ma,
                vm_now / 1000,
                (vm_now % 1000) / 10,
                temp_now
            );
        }

        if tick >= total_ticks {
            tim.bdtr().modify(|_, w| w.moe().clear_bit());
            en.set_low().ok();
            rprintln!(
                "DONE: {} s spin complete, stage disabled (re-run to spin again)",
                SPIN_SECONDS
            );
            loop {
                cortex_m::asm::nop();
            }
        }
    }
}
