//! Bounded open-loop six-step microscope; never controls timing from BEMF.
use super::*;
pub const N: usize = 384;
static DUTY_OVERRIDE: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
static APPLIED_DUTY: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
static PHASE_OVERRIDE: portable_atomic::AtomicI32 = portable_atomic::AtomicI32::new(0);
static APPLIED_PHASE: portable_atomic::AtomicI32 = portable_atomic::AtomicI32::new(0);
pub fn set_phase(degrees: i32) {
    PHASE_OVERRIDE.store(degrees, portable_atomic::Ordering::Relaxed);
}
pub fn set_duty(value: u32) {
    DUTY_OVERRIDE.store(value, portable_atomic::Ordering::Relaxed);
}
const SAMPLE_MIN_US: u32 = 100; // actual rate limited by measured trace+guard time
// Each slot is valid only if the comparator read is bracketed within
// [target,target+80] timer counts (1.25 us). Misses retained in row8.
const PWM_SLOTS: [u32; 12] = [
    128, 224, 320, 448, 640, 960, 1600, 2400, 3200, 4000, 4800, 5600,
];
#[inline(always)]
fn read_slot(target: u32, wait: u16) -> (u32, bool, u32) {
    let tim = unsafe { &*stm32::TIM1::ptr() };
    while tim.cnt().read().bits() < target {
        if t17().wrapping_sub(wait) > 200 {
            return (6400, false, 6400);
        }
    }
    let before = tim.cnt().read().bits();
    let value = unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 };
    (before, value, tim.cnt().read().bits())
}
static mut RECORDS: [[u16; 16]; N] = [[0; 16]; N];
static TRACE_TIMEOUTS: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);
use super::phase_schedule::STEPS as STEP_LUT;

pub fn step_for_phase(theta: u32) -> u8 {
    STEP_LUT[(theta >> 24) as usize]
}
pub fn run<F: FnMut() -> bool>(theta: u32, hz: u32, duty: u32, mut abort: F) -> (usize, u8, u32) {
    let phase_shift = PHASE_OVERRIDE
        .swap(0, portable_atomic::Ordering::Relaxed)
        .clamp(-60, 60);
    APPLIED_PHASE.store(phase_shift, portable_atomic::Ordering::Relaxed);
    let theta = campaign::phase_shift(theta, phase_shift);
    let override_duty = DUTY_OVERRIDE.swap(0, portable_atomic::Ordering::Relaxed);
    let duty = if override_duty == 0 {
        duty
    } else {
        override_duty
    }
    .min(MAX_DUTY_TENTHS);
    APPLIED_DUTY.store(duty, portable_atomic::Ordering::Relaxed);
    let mut step = STEP_LUT[(theta >> 24) as usize];
    let started = clock_us();
    let mut plan = sixstep_apply(step, duty);
    let mut sector_start = clock_us();
    core_bench::observe_begin(step, hz, started);
    wave_timer::start_six(theta, hz, duty);
    core_bench::polling_start();
    core_bench::observe_irq_start();
    let mut last_sample = started.wrapping_sub(SAMPLE_MIN_US);
    let mut count = 0;
    TRACE_TIMEOUTS.store(0, portable_atomic::Ordering::Relaxed);
    let mut reason = 1;
    let mut detector = detector::Detector::default();
    // Reference-timeout-sized diagnostic backstop, NOT a production tracking
    // test: at 200eHz this slow sampler has at most three consecutive valid
    // samples/sector (Entry083), but Detector needs at least four. Thus zero
    // candidates cannot diagnose rotor loss. Keep this bounded until replaced
    // by qualified production-event timing; do not silently weaken it.
    let mut event_count = 0u32;
    let mut event_watch =
        event_watch::Watch::new(started, 0, minz_core::am32_loop::BEMF_TIMEOUT_TICKS / 2);
    let bus0 = unsafe { adc_read(6) } as u32;
    let tim = unsafe { &*stm32::TIM1::ptr() };
    'capture: while clock_us().wrapping_sub(started) < 96_000 && count < N {
        let now = clock_us();
        if abort() {
            reason = 3;
            break;
        }
        if !get_idr(1, 14) {
            reason = 2;
            break;
        }
        if wave_timer::fault() != 0 {
            reason = wave_timer::fault();
            break;
        }
        if event_watch.poll(now, event_count).is_some() {
            reason = 6;
            break;
        }
        let (desired, sector_time) = wave_timer::physical();
        if desired != step {
            step = desired;
            plan = sixstep::plan(step, duty).unwrap();
            sector_start = clock_us().wrapping_sub(t17().wrapping_sub(sector_time) as u32);
        }
        if now.wrapping_sub(last_sample) < SAMPLE_MIN_US {
            continue;
        }
        let mut row = [0u16; 16];
        let adc = unsafe { &*stm32::ADC::ptr() };
        // Digital-only experiment: do not acquire the high-impedance neutral
        // or VC node. Current/bus/VREF guards remain below, after OFF read.
        let wait = t17();
        while tim.cnt().read().bits() < 500 {
            if t17().wrapping_sub(wait) > 200 {
                trace_timeout();
                continue 'capture;
            }
        }
        while !(32..64).contains(&tim.cnt().read().bits()) {
            if t17().wrapping_sub(wait) > 200 {
                trace_timeout();
                continue 'capture;
            }
        }
        let cnt = tim.cnt().read().bits();
        let on_value = unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 };
        // One raw timer read here; defer 32-bit clock extension/bookkeeping
        // until after the trace so it cannot consume the short ON window.
        let early_time = t17();
        let first = read_slot(128, wait);
        let late = read_slot(320, wait);
        let pair_end = late.2;
        let mut levels = (first.1 as u16) | ((late.1 as u16) << 2);
        let mut misses = (1u16 << 1) | (1 << 3);
        if first.0 < 128 || first.2 > 208 || first.2 < first.0 {
            misses |= 1;
        }
        if late.0 < 320 || late.2 > 400 || late.2 < late.0 {
            misses |= 4;
        }
        for (i, target) in PWM_SLOTS.iter().enumerate() {
            // 1.5-us adjacent slots overran on M0. Preserve wire positions,
            // explicitly mark these omitted rather than fabricate samples.
            if i < 4 {
                continue;
            }
            while tim.cnt().read().bits() < *target {
                if t17().wrapping_sub(wait) > 200 {
                    trace_timeout();
                    continue 'capture;
                }
            }
            let before = tim.cnt().read().bits();
            let value = unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 };
            let after = tim.cnt().read().bits();
            if value {
                levels |= 1 << i;
            }
            if before < *target || after > *target + 80 || after < before {
                misses |= 1 << i;
            }
        }
        let late_value = levels & (1 << 2) != 0;
        let off_value = levels & (1 << 6) != 0;
        let epoch = clock_us();
        let sample_time = epoch.wrapping_sub(t17().wrapping_sub(early_time) as u32);
        last_sample = sample_time;
        let age = sample_time.wrapping_sub(sector_start);
        unsafe {
            adc.smpr().write(|w| w.bits(7));
        }
        let elapsed = sample_time.wrapping_sub(started);
        row[0] = elapsed as u16;
        row[1] = (elapsed >> 16) as u16;
        row[2] = age as u16;
        row[3] = step as u16;
        row[4] = plan.floating as u16;
        row[5] = cnt as u16;
        row[6] = (off_value as u16)
            | ((on_value as u16) << 1)
            | ((get_idr(1, 14) as u16) << 2)
            | ((late_value as u16) << 3)
            | (1 << 4);
        row[7] = levels; // v6 digital PWM-slot mask, NOT ADC voltage
        row[8] = misses;
        row[15] = pair_end as u16;
        let currents = current_read_rotated(count);
        row[9..12].copy_from_slice(&currents);
        row[12] = unsafe { adc_read(6) };
        row[13] = unsafe { adc_read(13) };
        // Rejection bits: commutation/mux blank, ADC pair outside ON window,
        // slow telemetry scan crosses wrap (bit2, not BEMF rejection),
        // BEMF ADC pair crosses wrap (bit3). Raw observations always retained.
        row[14] = u16::from(age < 200)
            | (u16::from(cnt < 26 || pair_end >= duty * 64 / 10 || pair_end < cnt) << 1);
        if tim.cnt().read().bits() < row[5] as u32 {
            row[14] |= 4;
        }
        if row[15] < row[5] {
            row[14] |= 8;
        }
        // An IRQ commutation/mux change makes this whole scan mixed-phase.
        // Retain it, but never feed it to either detector.
        if wave_timer::physical().0 != step {
            row[14] |= 8;
        }
        let decision = detector.update(
            step,
            !late_value,
            row[14] & 11 == 0 && misses & 4 == 0 && row[6] & 4 != 0,
        );
        if decision.event {
            event_count = event_count.wrapping_add(1);
        }
        row[6] |= ((decision.expected as u16) << 5)
            | ((decision.armed as u16) << 6)
            | ((decision.event as u16) << 7)
            | ((decision.latched as u16) << 8);
        row[14] |= (decision.reason as u16) << 4;
        unsafe {
            core::ptr::addr_of_mut!(RECORDS)
                .cast::<[u16; 16]>()
                .add(count)
                .write(row);
        }
        count += 1;
        if currents.iter().any(|&v| (v as i32 - 2048).abs() > 1200) {
            reason = 4;
            break;
        }
        if (row[12] as u32) * 10 < bus0 * 7 {
            reason = 5;
            break;
        }
        core_bench::observe_sample(
            step,
            late_value,
            row[14] & 11 == 0 && misses & 4 == 0 && row[6] & 4 != 0,
        );
    }
    gates_off();
    set_pin(3, 1, false);
    core_bench::observe_end();
    let elapsed = clock_us().wrapping_sub(started);
    // Restore the known sine timer setup while driver is disabled.
    unsafe {
        tim.ccmr1_output().write(|w| w.bits(0x6868));
        tim.ccmr2_output().write(|w| w.bits(0x68));
        tim.ccer().write(|w| w.bits(0x555));
        tim.egr().write(|w| w.bits(1));
    }
    (count, reason, elapsed)
}

pub fn timing_summary<W: Write>(out: &mut W, count: usize) {
    core_bench::observe_summary(out, false);
    let mut misses = [0u16; 12];
    let mut rejected = 0;
    let mut minimum = 6400;
    let mut maximum = 0;
    for i in 0..count {
        let row = unsafe {
            core::ptr::addr_of!(RECORDS)
                .cast::<[u16; 16]>()
                .add(i)
                .read()
        };
        for j in 0..12 {
            if row[8] & (1 << j) != 0 {
                misses[j] += 1;
            }
        }
        if row[14] & 2 != 0 {
            rejected += 1;
        }
        minimum = minimum.min(row[15]);
        maximum = maximum.max(row[15]);
    }
    let _ = writeln!(
        out,
        "OBSTIMING n={} late_min={} late_max={} on_reject={} misses={:?} en=0 moe=0",
        count, minimum, maximum, rejected, misses
    );
}
fn trace_timeout() {
    TRACE_TIMEOUTS.fetch_add(1, portable_atomic::Ordering::Relaxed);
}

pub fn dump<W: Write>(out: &mut W, count: usize) {
    let _ = writeln!(
        out,
        "OBSPHASE degrees={} one_shot=1 positive_advances_command=1",
        APPLIED_PHASE.load(portable_atomic::Ordering::Relaxed)
    );
    let _ = writeln!(
        out,
        "OBSDUTY tenths={} override_one_shot=1",
        APPLIED_DUTY.load(portable_atomic::Ordering::Relaxed)
    );
    core_bench::trace_dump(out);
    let _ = writeln!(
        out,
        "OBSLOSS trace_timeouts={}",
        TRACE_TIMEOUTS.load(portable_atomic::Ordering::Relaxed)
    );
    // v8 retains raw bits, but decision flags use inverted reference-HAL level.
    let _ = writeln!(
        out,
        "OBS n={} wire=a85-v8 fields=us_lo,us_hi,sector_us,step,float,cnt,flags,vc,neutral,ia,ib,ic,bus,vref,reject,cnt_end",
        count
    );
    for i in 0..count {
        let row = unsafe {
            core::ptr::addr_of!(RECORDS)
                .cast::<[u16; 16]>()
                .add(i)
                .read()
        };
        let _ = snapshot::record(out, "B85", &row);
    }
    let _ = writeln!(out, "OBS END");
}
