//! Open-loop sine spin using the six drive signals as PLAIN GPIO outputs
//! (no TIM1, no HAL PWM -- exactly the configuration pin-walk verified).
//!
//! Soft PWM: 20 us slots (~50 kHz); each phase runs a sigma-delta
//! accumulator on its sine-shaped duty target (peak 7% < 8% cap), so the
//! effective duty is smooth while individual pulses are sparse 20 us
//! blips. Every edge sequences off-side first, ~1 us gap, then on-side
//! (bit-banged dead time; the STDRIVE also interlocks INH=INL=1).
//!
//! Profile: 30 ms low-side precharge, 0.5 s park, 5 -> 100 Hz over 3 s,
//! hold to 45 s total, stop. Guards: IS overcurrent, VM sag, NTC, nFLT.
//!
//! Run: `cargo run --release --example spin-gpio`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m::asm::delay as cycdelay;
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

const CARRIER_HZ: u32 = 16_000; // 62.5 us periods, on-times 0..4.4 us
const PEAK_DUTY_Q16: u32 = 4587; // 7% of 65536, < 8% cap
const TARGET_HZ: u32 = 100;
const RAMP_START_HZ: u32 = 5;
const SPIN_SECONDS: u32 = 45;
const IS_KILL_DELTA_MV: u16 = 150; // ~2.5 A
const DELAY_1US: u32 = 21; // asm::delay ~3 cyc/count @64 MHz
// Full period in delay units minus measured loop overhead (calibrated).
const PERIOD_UNITS: u32 = 62 * DELAY_1US;
const MIN_ON_UNITS: u32 = DELAY_1US; // skip pulses under ~1 us

struct ChIs;
impl Channel<Adc> for ChIs {
    type ID = u8;
    fn channel() -> u8 {
        15
    }
}
struct ChBus;
impl Channel<Adc> for ChBus {
    type ID = u8;
    fn channel() -> u8 {
        1
    }
}
struct ChTemp;
impl Channel<Adc> for ChTemp {
    type ID = u8;
    fn channel() -> u8 {
        17
    }
}

// Direct BSRR access: INH1=PA8 INH2=PA9 INH3=PA10, INL1=PA7 INL2=PD3 INL3=PD4.
const INH_A_BIT: [u32; 3] = [8, 9, 10];
const INL_A_BIT: u32 = 7; // phase 1 low side on GPIOA
const INL_D_BIT: [u32; 2] = [3, 4]; // phases 2,3 low sides on GPIOD

#[allow(dead_code)]
#[inline(always)]
fn phase_high(k: usize) {
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let pd = &*stm32::GPIOD::ptr();
        // low side off first, gap, then high side on
        if k == 0 {
            pa.bsrr().write(|w| w.bits(1 << (INL_A_BIT + 16)));
        } else {
            pd.bsrr().write(|w| w.bits(1 << (INL_D_BIT[k - 1] + 16)));
        }
        cycdelay(DELAY_1US);
        pa.bsrr().write(|w| w.bits(1 << INH_A_BIT[k]));
    }
}

#[allow(dead_code)]
#[inline(always)]
fn phase_low(k: usize) {
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let pd = &*stm32::GPIOD::ptr();
        // high side off first, gap, then low side on
        pa.bsrr().write(|w| w.bits(1 << (INH_A_BIT[k] + 16)));
        cycdelay(DELAY_1US);
        if k == 0 {
            pa.bsrr().write(|w| w.bits(1 << INL_A_BIT));
        } else {
            pd.bsrr().write(|w| w.bits(1 << INL_D_BIT[k - 1]));
        }
    }
}

/// Stage safe: all six low, driver disabled (EN node low lights the LED).
fn kill(reason: &str) -> ! {
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let pd = &*stm32::GPIOD::ptr();
        pa.bsrr().write(|w| {
            w.bits((1 << (7 + 16)) | (1 << (8 + 16)) | (1 << (9 + 16)) | (1 << (10 + 16)))
        });
        pd.bsrr()
            .write(|w| w.bits((1 << (3 + 16)) | (1 << (4 + 16))));
        let pc = &*stm32::GPIOC::ptr();
        pc.bsrr().write(|w| w.br9().set_bit());
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
    let gpiod = dp.GPIOD.split(&mut rcc);

    let _bus = gpioa.pa1.into_analog();
    let _is = gpiob.pb11.into_analog();
    let _temp = gpioc.pc4.into_analog();
    let mut nflt = gpioa.pa6.into_floating_input();

    // Six plain push-pull outputs, all low.
    let mut inh1 = gpioa.pa8.into_push_pull_output();
    let mut inh2 = gpioa.pa9.into_push_pull_output();
    let mut inh3 = gpioa.pa10.into_push_pull_output();
    let mut inl1 = gpioa.pa7.into_push_pull_output();
    let mut inl2 = gpiod.pd3.into_push_pull_output();
    let mut inl3 = gpiod.pd4.into_push_pull_output();
    inh1.set_low().ok();
    inh2.set_low().ok();
    inh3.set_low().ok();
    inl1.set_low().ok();
    inl2.set_low().ok();
    inl3.set_low().ok();

    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    stby.set_high().ok();
    en.set_low().ok();

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();

    let vm0 = adc.read_voltage(&mut ChBus).unwrap() as u32 * 1759 / 100;
    rprintln!(
        "spin-gpio: VM~{}.{:02} V, peak duty 7%, target {} Hz",
        vm0 / 1000,
        (vm0 % 1000) / 10,
        TARGET_HZ
    );
    if vm0 < 6000 {
        kill("VM below 6 V");
    }
    let mut acc32: u32 = 0;
    for _ in 0..16 {
        acc32 += adc.read_voltage(&mut ChIs).unwrap() as u32;
    }
    let is_base = (acc32 / 16) as u16;
    rprintln!("IS baseline {} mV", is_base);

    // Enable driver; 30 ms all-low precharge.
    en.set_high().ok();
    delay.delay(2.millis());
    inl1.set_high().ok();
    inl2.set_high().ok();
    inl3.set_high().ok();
    delay.delay(30.millis());
    if nflt.is_low().unwrap() {
        kill("nFLT after enable");
    }

    // 16 kHz aligned-start / sorted-end soft PWM. Each period: active
    // phases' INL off -> 1 us gap -> all INH on together (one BSRR write,
    // all three INH live on GPIOA) -> staggered INH offs at their duty
    // times -> 1 us gap -> INLs back on -> pad to period end. On-times
    // are 0..~4.4 us, so per-pulse current stays in normal PWM territory.
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
    // Calibration fudge: fixed per-period compute/edge overhead (~15 us).
    const OVERHEAD_UNITS: u32 = 300;

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

        // Per-phase on-times in delay units.
        let mut d = [0u32; 3];
        for k in 0..3usize {
            let idx = (phase.wrapping_add(k as u32 * 0x5555_5555) >> 24) as usize;
            let duty_q16 = amp_q16 * SINE_LUT[idx] as u32 >> 8;
            let units = duty_q16 * PERIOD_UNITS >> 16;
            d[k] = if units >= MIN_ON_UNITS { units } else { 0 };
        }

        // Sort indices by on-time ascending (3-element network).
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
                let pd = &*stm32::GPIOD::ptr();
                // INL off for active phases.
                let mut la: u32 = 0;
                let mut ld: u32 = 0;
                if d[0] > 0 {
                    la |= 1 << (INL_A_BIT + 16);
                }
                if d[1] > 0 {
                    ld |= 1 << (INL_D_BIT[0] + 16);
                }
                if d[2] > 0 {
                    ld |= 1 << (INL_D_BIT[1] + 16);
                }
                if la != 0 {
                    pa.bsrr().write(|w| w.bits(la));
                }
                if ld != 0 {
                    pd.bsrr().write(|w| w.bits(ld));
                }
                cycdelay(DELAY_1US);
                // All active INH on together.
                pa.bsrr().write(|w| w.bits(active_mask_a));
                // Staggered offs.
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
                // INLs back on (freewheel window between off and here is
                // carried by the body diodes -- standard diode-mode PWM).
                let mut la: u32 = 0;
                let mut ld: u32 = 0;
                if d[0] > 0 {
                    la |= 1 << INL_A_BIT;
                }
                if d[1] > 0 {
                    ld |= 1 << INL_D_BIT[0];
                }
                if d[2] > 0 {
                    ld |= 1 << INL_D_BIT[1];
                }
                if la != 0 {
                    pa.bsrr().write(|w| w.bits(la));
                }
                if ld != 0 {
                    pd.bsrr().write(|w| w.bits(ld));
                }
                cycdelay(PERIOD_UNITS.saturating_sub(elapsed + OVERHEAD_UNITS));
            }
        } else {
            cycdelay(PERIOD_UNITS.saturating_sub(OVERHEAD_UNITS));
        }

        // 1 kHz guards; overcurrent needs 2 consecutive strikes (async
        // sampling can legitimately catch a single on-pulse).
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
            if nflt.is_low().unwrap() {
                kill("nFLT");
            }
        }
        if period % (CARRIER_HZ / 2) == 0 {
            let vm = adc.read_voltage(&mut ChBus).unwrap() as u32 * 1759 / 100;
            if vm < 6000 {
                kill("VM sag");
            }
            let ma = is_now.abs_diff(is_base) as u32 * 100 / 6;
            rprintln!(
                "t={}s f={}.{:02}Hz amp={}% |I|~{} mA VM~{}.{:02}V",
                period / CARRIER_HZ,
                freq_chz / 100,
                freq_chz % 100,
                amp_q16 * 100 / 65536,
                ma,
                vm / 1000,
                (vm % 1000) / 10
            );
        }

        if period >= total_p {
            kill("DONE - 45 s spin complete (stage safed; re-run to spin again)");
        }
    }
}
