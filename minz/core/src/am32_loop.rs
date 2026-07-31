//! AM32 main-loop / tenKhzRoutine band functions over the cohesion
//! clusters — the host-testable half of `examples/am32_clone.rs`. All
//! 0.5 µs / 0..2000-domain quantities; every constant and branch cites
//! its AM32 line.

use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

use crate::am32::{self, Am32Intervals, UartCmd};

// ===============================================================
// AM32 constants — factory-default EEPROM, VIMDRONES_L431.
// (Values that depend on the flashed EEPROM image are flagged
//  BENCH-VERIFY; the build is correct regardless of their tuning.)
// ===============================================================

/// AM32's throttle/duty domain is 0..2000 (main.c throughout).
pub const DUTY_FULL: u16 = 2000;

/// tenKhzRoutine cadence. AM32 targets.h:177 `LOOP_FREQUENCY_HZ 20000`.
pub const LOOP_FREQUENCY_HZ: u32 = 20_000;

/// AM32 targets.h:5300 `TARGET_MIN_BEMF_COUNTS 3`.
pub const TARGET_MIN_BEMF_COUNTS: u16 = 3;
/// AM32 main.c:550 `bad_count_threshold = CPU_FREQUENCY_MHZ/24` = 80/24.
pub const BAD_COUNT_THRESHOLD: u16 = 80 / 24; // = 3

/// Polling<->interrupt changeover, 0.5 µs ticks. AM32 targets.h:5344
/// `POLLING_MODE_THRESHOLD 2000`; non-bi-dir keeps it whole
/// (loadEEpromSettings main.c:795).
pub const POLLING_MODE_CHANGEOVER: u32 = 2000;

/// Constant 15° advance: AUTO_ADVANCE off, advance_level default → 16
/// (loadEEpromSettings main.c:628-630); `advance = ci*16>>6 = ci/4`.
pub const TEMP_ADVANCE: u32 = 16;

/// Startup interval seed, 0.5 µs ticks. AM32 startMotor main.c:954.
pub const STARTUP_INTERVAL_TICKS: u32 = 10_000;
/// AM32 main.c:575 global init `commutation_interval = 12500`.
pub const INIT_INTERVAL_TICKS: u32 = 12_500;

// --- Duty pipeline defaults (loadEEpromSettings main.c:610-797) ---
// BENCH-VERIFY: these three resolve from the flashed EEPROM bytes,
// which are not in the source tree. Values below follow the code path
// for a fresh/default image (driving_brake_strength defaults to 10 at
// main.c:696-698, so the dead-time-override block main.c:700-721 is
// skipped) with the documented AM32-configurator factory bytes.
/// main.c:428 `minimum_duty_cycle = DEAD_TIME` (VIMDRONES DEAD_TIME=45).
pub const MINIMUM_DUTY_CYCLE: u16 = 45;
/// main.c:653 `min_startup_duty = minimum_duty_cycle + startup_power`
/// (startup_power factory ≈ 100).
pub const MIN_STARTUP_DUTY: u16 = MINIMUM_DUTY_CYCLE + 100; // 145
/// main.c:657 `startup_max_duty_cycle = minimum_duty_cycle + 400`.
pub const STARTUP_MAX_DUTY_CYCLE: u16 = MINIMUM_DUTY_CYCLE + 400; // 445

// Ramp rates (main.c:1746-1754) + low-rpm duty ceiling map
// (main.c:436-439) live in `crate::am32` (with their AM32
// citations); the pipeline helpers there consume them.

/// main.c:390 `low_rpm_throttle_limit = 1` (on by default).
pub const LOW_RPM_THROTTLE_LIMIT: bool = true;

/// variable_pwm carrier ride (main.c:2192-2195). BENCH-VERIFY: the
/// factory `eepromBuffer.variable_pwm` byte is EEPROM-resident; the
/// mode-1 path is transliterated and enabled here.
pub const VARIABLE_PWM: u8 = 1;

/// bemf timeout re-kick threshold, 0.5 µs ticks. main.c:2495
/// `INTERVAL_TIMER_COUNT > 45000`.
pub const BEMF_TIMEOUT_TICKS: u32 = 45_000;

/// UART deadman: no command for 3 s → throttle 0. main.c:1425.
pub const UART_DEADMAN_LIMIT: u32 = 3 * LOOP_FREQUENCY_HZ; // 60000 ticks

// --- ISR-delay injection rig (bench-only causal test, 2026-07-26) ---
// The deaf-window PRIMASK hypothesis test bed: the TIM6 (priority 3)
// trampoline busy-waits an adjustable number of CPU cycles, split by
// whether it runs INSIDE a `cortex_m::interrupt::free` critical section
// (masks COMP) or OUTSIDE (COMP can preempt). Cycles → µs at 80 MHz:
// 80 cyc/µs. Both amounts default 0 (inert).
/// Per-keypress bump for either injected delay, in CPU cycles
/// (80 MHz → 280 cyc ≈ 3.5 µs).
pub const DELAY_BUMP_CYC: u32 = 280;
/// Hard cap on either injected delay, in CPU cycles (4000 cyc ≈ 50 µs)
/// — a fat-finger can't wedge the tick under the 1 s IWDG. Both the
/// apply-side clamp here AND the trampoline `.min()` enforce it.
pub const DELAY_CAP_CYC: u32 = 4000;

// ===============================================================
// Cohesion clusters — each struct bundles the loose firmware statics
// that are written/read together, holding ONLY `&Atomic*` refs
// (storage stays with the caller). Every servicing fn takes the
// clusters it touches as `&` params.
// ===============================================================

/// Scheduling / ZC-estimate cluster (all 0.5 µs units).
pub struct Sched<'a> {
    pub commutation_interval: &'a AtomicU32,
    pub interval_hist: &'a [AtomicU32; 6],
    pub average_interval: &'a AtomicU32,
    pub last_average_interval: &'a AtomicU32,
    pub last_zc: &'a AtomicU16,
    pub this_zc: &'a AtomicU16,
    pub wait_time: &'a AtomicU16,
}

/// Commutation / run-state cluster + its event counters.
pub struct Drive<'a> {
    pub current_step: &'a AtomicU16,
    pub rising: &'a AtomicBool,
    pub old_routine: &'a AtomicBool,
    pub running: &'a AtomicBool,
    pub zcfound: &'a AtomicBool,
    pub bemf_counter: &'a AtomicU16,
    pub min_bemf_up: &'a AtomicU16,
    pub min_bemf_down: &'a AtomicU16,
    pub zero_crosses: &'a AtomicU32,
    pub filter_level: &'a AtomicU16,
    pub bad_count: &'a AtomicU16,
    pub desync_check: &'a AtomicBool,
    pub desync_happened: &'a AtomicU32,
    pub bemf_timeout_happened: &'a AtomicU32,
    pub tenkhz_counter: &'a AtomicU16,
    pub zcfr_guard_hits: &'a AtomicU32,
}

/// Duty pipeline cluster (0..2000 domain) + the latched kill.
pub struct Duty<'a> {
    pub input: &'a AtomicU16,
    pub adjusted_input: &'a AtomicU16,
    pub uart_duty_input: &'a AtomicU16,
    pub duty_cycle_setpoint: &'a AtomicU16,
    pub duty_cycle: &'a AtomicU16,
    pub last_duty_cycle: &'a AtomicU16,
    pub duty_cycle_maximum: &'a AtomicU16,
    pub ramp_count: &'a AtomicU16,
    pub killed: &'a AtomicBool,
    pub kill_reason: &'a AtomicU16,
    /// AM32's `tim1_arr` VARIABLE (main.c:434) — the live carrier ARR
    /// tracked as state, exactly like AM32: variable_pwm writes it
    /// (main.c:2192-2195), the tenKhz duty apply reads it for the
    /// `duty*tim1_arr/2000` rescale (main.c:1790-1791). rm32's
    /// `PwmOutput` has NO ARR getter, so a register readback is not a
    /// seam — this shadow is the rung-2 substitution for the old
    /// `MotorPwm::max_duty()` live TIM1.ARR read.
    pub tim1_arr: &'a AtomicU16,
}

/// Observer / bench cluster (ADC harvest, OC accumulator, req flags).
pub struct Bench<'a> {
    pub uart_deadman_ticks: &'a AtomicU32,
    pub i_raw: &'a AtomicU16,
    pub vbat_raw: &'a AtomicU16,
    pub oc_acc: &'a AtomicU32,
    pub oc_cnt: &'a AtomicU32,
    /// consecutive TIM6 ticks with vbat below the floor (debounce).
    pub vbat_low_ticks: &'a AtomicU32,
    /// boot-latched kill floor = 70% of the first vbat harvest;
    /// 0 = not yet latched.
    pub vbat_floor_raw: &'a AtomicU16,
    pub stop_req: &'a AtomicBool,
    pub dump_req: &'a AtomicBool,
    pub info_req: &'a AtomicBool,
    /// 'G' — GECKO free-run current-ring capture request (main-context
    /// oversample + dump, then back to inject-only).
    pub gecko_req: &'a AtomicBool,
    /// 'X' — WAXWING-lite phase-voltage-ring dump request (main-context
    /// dump of the always-on TIM6 rings; nothing to start or stop).
    pub wax_req: &'a AtomicBool,
    /// 'H' — per-ISR duration histogram dump request (main-context dump
    /// of the always-on TIM6/TIM16/COMP bin arrays).
    pub hist_req: &'a AtomicBool,
    /// 'F' — toggle the free-run ADC oversample (rm32-like scan
    /// injector) continuously ON/OFF; reproduces the jitter on the
    /// controllable clone.
    pub freerun_req: &'a AtomicBool,
    pub zct_stream_on: &'a AtomicBool,
    /// ISR-delay injection rig (bench causal test for the deaf-window
    /// PRIMASK hypothesis): busy-wait cycles the TIM6 tick executes
    /// INSIDE a `cortex_m::interrupt::free` critical section (masks
    /// COMP). Bumped by ']'/'[', clamped [0, [`DELAY_CAP_CYC`]].
    pub delay_in_free: &'a AtomicU32,
    /// Companion: busy-wait cycles the TIM6 tick executes OUTSIDE any
    /// critical section (COMP can preempt). Bumped by '\''/';'.
    pub delay_out_free: &'a AtomicU32,
}

impl Sched<'_> {
    /// The 6-slot `commutation_intervals[]` history over the firmware
    /// statics (minz_core owns the push/sum/e_com_time logic) —
    /// main.c:441,887.
    #[inline]
    pub fn intervals(&self) -> Am32Intervals<'_> {
        Am32Intervals::new(self.interval_hist)
    }
}

// ===============================================================
// Pure while(1)/tenKhz band fns (no hardware, no black box) —
// the host-testable bands of the example's orchestrators.
// ===============================================================

/// min_bemf_counts schedule band (main.c:2177-2188, non-bi-dir).
#[inline]
pub fn min_bemf_schedule(drive: &Drive) {
    if drive.zero_crosses.load(Ordering::Relaxed) < 5 {
        drive.min_bemf_up.store(TARGET_MIN_BEMF_COUNTS * 2, Ordering::Relaxed);
        drive.min_bemf_down.store(TARGET_MIN_BEMF_COUNTS * 2, Ordering::Relaxed);
    } else {
        drive.min_bemf_up.store(TARGET_MIN_BEMF_COUNTS, Ordering::Relaxed);
        drive.min_bemf_down.store(TARGET_MIN_BEMF_COUNTS, Ordering::Relaxed);
    }
}

/// average_interval band (main.c:2283): average_interval = e_com_time / 3.
#[inline]
pub fn store_average_interval(sched: &Sched, e_com_time: i32) -> u32 {
    let average_interval = if e_com_time > 0 { (e_com_time / 3) as u32 } else { 0 };
    sched.average_interval.store(average_interval, Ordering::Relaxed);
    average_interval
}

/// low-rpm duty ceiling + filter_level map band (main.c:2441-2469).
/// Returns the single `running`/`zero_crosses` loads that the bemf-timeout
/// bands reuse (preserves the original one-load ordering).
#[inline]
pub fn filter_and_duty_max(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    e_com_time: i32,
    average_interval: u32,
) -> (bool, u32) {
    let running = drive.running.load(Ordering::Relaxed);
    let duty_max = am32::low_rpm_duty_ceiling(e_com_time, running, LOW_RPM_THROTTLE_LIMIT);
    duty.duty_cycle_maximum.store(duty_max, Ordering::Relaxed);

    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    let ci = sched.commutation_interval.load(Ordering::Relaxed);
    let mut filter = if zc < 100 && ci > 500 {
        12
    } else {
        am32::map(average_interval as i32, 100, 500, 3, 12)
    };
    if ci < 50 {
        filter = 2;
    }
    drive.filter_level.store(filter as u16, Ordering::Relaxed);
    (running, zc)
}

/// bemf-timeout leniency reset band (main.c:2261-2273).
#[inline]
pub fn bemf_timeout_resets(drive: &Drive, duty: &Duty, zc: u32) {
    let adj = duty.adjusted_input.load(Ordering::Relaxed);
    if zc > 1000 || adj == 0 {
        drive.bemf_timeout_happened.store(0, Ordering::Relaxed);
    }
    if zc > 100 && adj < 200 {
        drive.bemf_timeout_happened.store(0, Ordering::Relaxed);
    }
}

/// setInput setpoint-map + startup/max clamp band (main.c:1302-1314).
#[inline]
pub fn set_input_clamp(drive: &Drive, duty: &Duty, input: u16) {
    // duty_cycle_setpoint = map(input, 47, 2047, minimum_duty_cycle, 2000)
    // when armed, else 0 (am32::duty_setpoint). Stored here (mirrors the
    // two per-branch stores main.c:1194/1206) then re-clamped below —
    // the intermediate value is what a preempting TIM6 tick reads.
    duty.duty_cycle_setpoint.store(
        am32::duty_setpoint(input, MINIMUM_DUTY_CYCLE, DUTY_FULL),
        Ordering::Relaxed,
    );
    let mut sp = duty.duty_cycle_setpoint.load(Ordering::Relaxed);

    // startup clamp (main.c:1302-1314), non-bi-dir (30 >> 0 = 30).
    if input >= 47 && drive.zero_crosses.load(Ordering::Relaxed) < 30 {
        if sp < MIN_STARTUP_DUTY {
            sp = MIN_STARTUP_DUTY;
        }
        if sp > STARTUP_MAX_DUTY_CYCLE {
            sp = STARTUP_MAX_DUTY_CYCLE;
        }
    }
    let dmax = duty.duty_cycle_maximum.load(Ordering::Relaxed);
    if sp > dmax {
        sp = dmax;
    }
    duty.duty_cycle_setpoint.store(sp, Ordering::Relaxed);
}

/// Apply a decoded UART command (am32::UartDuty parses; this performs
/// the side effects) — uart_duty_poll main.c:1367-1416 + bench keys.
#[inline]
pub fn apply_uart_cmd(duty: &Duty, bench: &Bench, cmd: Option<UartCmd>) {
    match cmd {
        // main.c:1376-1381,1397-1398 stop ('s'/'w' and a committed 0):
        // zero throttle + adjusted + deadman and request the bench stop.
        Some(UartCmd::Stop) => {
            duty.uart_duty_input.store(0, Ordering::Relaxed);
            duty.adjusted_input.store(0, Ordering::Relaxed);
            bench.uart_deadman_ticks.store(0, Ordering::Relaxed);
            bench.stop_req.store(true, Ordering::Relaxed);
        }
        Some(UartCmd::SetThrottle(inn)) => {
            duty.uart_duty_input.store(inn, Ordering::Relaxed);
            // adjusted_input mirror (main.c:1410)
            duty.adjusted_input.store(if inn <= 48 { 0 } else { inn }, Ordering::Relaxed);
            bench.uart_deadman_ticks.store(0, Ordering::Relaxed);
        }
        Some(UartCmd::TraceToggle) => {
            let on = !bench.zct_stream_on.load(Ordering::Relaxed);
            bench.zct_stream_on.store(on, Ordering::Relaxed);
        }
        Some(UartCmd::Info) => bench.info_req.store(true, Ordering::Relaxed),
        Some(UartCmd::BbDump) => bench.dump_req.store(true, Ordering::Relaxed),
        Some(UartCmd::GeckoDump) => bench.gecko_req.store(true, Ordering::Relaxed),
        Some(UartCmd::WaxDump) => bench.wax_req.store(true, Ordering::Relaxed),
        Some(UartCmd::HistDump) => bench.hist_req.store(true, Ordering::Relaxed),
        Some(UartCmd::FreeRunToggle) => bench.freerun_req.store(true, Ordering::Relaxed),
        // ISR-delay injection rig: bump the in/out-critical-section
        // busy-wait, clamped [0, DELAY_CAP_CYC].
        Some(UartCmd::DelayInFreeUp) => bump_delay(bench.delay_in_free, DELAY_BUMP_CYC as i32),
        Some(UartCmd::DelayInFreeDown) => bump_delay(bench.delay_in_free, -(DELAY_BUMP_CYC as i32)),
        Some(UartCmd::DelayOutFreeUp) => bump_delay(bench.delay_out_free, DELAY_BUMP_CYC as i32),
        Some(UartCmd::DelayOutFreeDown) => bump_delay(bench.delay_out_free, -(DELAY_BUMP_CYC as i32)),
        None => {}
    }
}

/// Bump an injected-delay cell by `delta` cycles, clamped to
/// [0, [`DELAY_CAP_CYC`]] (the ISR-delay rig; `apply_uart_cmd` helper).
#[inline]
fn bump_delay(cell: &AtomicU32, delta: i32) {
    let cur = cell.load(Ordering::Relaxed) as i32;
    let next = (cur + delta).clamp(0, DELAY_CAP_CYC as i32);
    cell.store(next as u32, Ordering::Relaxed);
}

/// Duty ramp band (main.c:1736-1791). ramp_divider=0 → every tick.
#[inline]
pub fn duty_ramp(sched: &Sched, drive: &Drive, duty: &Duty, setpoint: i32) -> u16 {
    let _ = duty.ramp_count.fetch_add(1, Ordering::Relaxed);
    let last = duty.last_duty_cycle.load(Ordering::Relaxed) as i32;
    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    let avg = sched.average_interval.load(Ordering::Relaxed);
    // Ramp rate selection + one-sided step clamp + 0..2000 domain
    // clamp (am32::ramp_rate / ramp_toward).
    let rate = am32::ramp_rate(zc, last, avg);
    let duty_val = am32::ramp_toward(last, setpoint, rate);
    duty.duty_cycle.store(duty_val, Ordering::Relaxed);
    duty_val
}

/// UART deadman band (main.c:1425): 3 s without a command → throttle 0.
#[inline]
pub fn uart_deadman_tick(duty: &Duty, bench: &Bench) {
    let dm = bench.uart_deadman_ticks.load(Ordering::Relaxed) + 1;
    if dm > UART_DEADMAN_LIMIT {
        duty.uart_duty_input.store(0, Ordering::Relaxed);
        duty.adjusted_input.store(0, Ordering::Relaxed);
        bench.uart_deadman_ticks.store(UART_DEADMAN_LIMIT + 1, Ordering::Relaxed);
    } else {
        bench.uart_deadman_ticks.store(dm, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Owned-atomics fixtures: each store owns the statics-shaped
    // storage; `.as_cluster()` borrows it as the firmware struct
    // (mirrors the example's static wiring).
    #[derive(Default)]
    struct SchedStore {
        commutation_interval: AtomicU32,
        interval_hist: [AtomicU32; 6],
        average_interval: AtomicU32,
        last_average_interval: AtomicU32,
        last_zc: AtomicU16,
        this_zc: AtomicU16,
        wait_time: AtomicU16,
    }
    impl SchedStore {
        fn sched(&self) -> Sched<'_> {
            Sched {
                commutation_interval: &self.commutation_interval,
                interval_hist: &self.interval_hist,
                average_interval: &self.average_interval,
                last_average_interval: &self.last_average_interval,
                last_zc: &self.last_zc,
                this_zc: &self.this_zc,
                wait_time: &self.wait_time,
            }
        }
    }

    #[derive(Default)]
    struct DriveStore {
        current_step: AtomicU16,
        rising: AtomicBool,
        old_routine: AtomicBool,
        running: AtomicBool,
        zcfound: AtomicBool,
        bemf_counter: AtomicU16,
        min_bemf_up: AtomicU16,
        min_bemf_down: AtomicU16,
        zero_crosses: AtomicU32,
        filter_level: AtomicU16,
        bad_count: AtomicU16,
        desync_check: AtomicBool,
        desync_happened: AtomicU32,
        bemf_timeout_happened: AtomicU32,
        tenkhz_counter: AtomicU16,
        zcfr_guard_hits: AtomicU32,
    }
    impl DriveStore {
        fn drive(&self) -> Drive<'_> {
            Drive {
                current_step: &self.current_step,
                rising: &self.rising,
                old_routine: &self.old_routine,
                running: &self.running,
                zcfound: &self.zcfound,
                bemf_counter: &self.bemf_counter,
                min_bemf_up: &self.min_bemf_up,
                min_bemf_down: &self.min_bemf_down,
                zero_crosses: &self.zero_crosses,
                filter_level: &self.filter_level,
                bad_count: &self.bad_count,
                desync_check: &self.desync_check,
                desync_happened: &self.desync_happened,
                bemf_timeout_happened: &self.bemf_timeout_happened,
                tenkhz_counter: &self.tenkhz_counter,
                zcfr_guard_hits: &self.zcfr_guard_hits,
            }
        }
    }

    #[derive(Default)]
    struct DutyStore {
        input: AtomicU16,
        adjusted_input: AtomicU16,
        uart_duty_input: AtomicU16,
        duty_cycle_setpoint: AtomicU16,
        duty_cycle: AtomicU16,
        last_duty_cycle: AtomicU16,
        duty_cycle_maximum: AtomicU16,
        ramp_count: AtomicU16,
        killed: AtomicBool,
        kill_reason: AtomicU16,
        tim1_arr: AtomicU16,
    }
    impl DutyStore {
        fn duty(&self) -> Duty<'_> {
            Duty {
                input: &self.input,
                adjusted_input: &self.adjusted_input,
                uart_duty_input: &self.uart_duty_input,
                duty_cycle_setpoint: &self.duty_cycle_setpoint,
                duty_cycle: &self.duty_cycle,
                last_duty_cycle: &self.last_duty_cycle,
                duty_cycle_maximum: &self.duty_cycle_maximum,
                ramp_count: &self.ramp_count,
                killed: &self.killed,
                kill_reason: &self.kill_reason,
                tim1_arr: &self.tim1_arr,
            }
        }
    }

    #[derive(Default)]
    struct BenchStore {
        uart_deadman_ticks: AtomicU32,
        i_raw: AtomicU16,
        vbat_raw: AtomicU16,
        oc_acc: AtomicU32,
        oc_cnt: AtomicU32,
        vbat_low_ticks: AtomicU32,
        vbat_floor_raw: AtomicU16,
        stop_req: AtomicBool,
        dump_req: AtomicBool,
        info_req: AtomicBool,
        gecko_req: AtomicBool,
        wax_req: AtomicBool,
        hist_req: AtomicBool,
        freerun_req: AtomicBool,
        zct_stream_on: AtomicBool,
        delay_in_free: AtomicU32,
        delay_out_free: AtomicU32,
    }
    impl BenchStore {
        fn bench(&self) -> Bench<'_> {
            Bench {
                uart_deadman_ticks: &self.uart_deadman_ticks,
                i_raw: &self.i_raw,
                vbat_raw: &self.vbat_raw,
                oc_acc: &self.oc_acc,
                oc_cnt: &self.oc_cnt,
                vbat_low_ticks: &self.vbat_low_ticks,
                vbat_floor_raw: &self.vbat_floor_raw,
                stop_req: &self.stop_req,
                dump_req: &self.dump_req,
                info_req: &self.info_req,
                gecko_req: &self.gecko_req,
                wax_req: &self.wax_req,
                hist_req: &self.hist_req,
                freerun_req: &self.freerun_req,
                zct_stream_on: &self.zct_stream_on,
                delay_in_free: &self.delay_in_free,
                delay_out_free: &self.delay_out_free,
            }
        }
    }

    #[test]
    fn min_bemf_schedule_doubles_below_five_zcs() {
        let ds = DriveStore::default();
        let drive = ds.drive();
        drive.zero_crosses.store(4, Ordering::Relaxed);
        min_bemf_schedule(&drive);
        assert_eq!(drive.min_bemf_up.load(Ordering::Relaxed), TARGET_MIN_BEMF_COUNTS * 2);
        assert_eq!(drive.min_bemf_down.load(Ordering::Relaxed), TARGET_MIN_BEMF_COUNTS * 2);
        // At 5 and above: back to the target.
        drive.zero_crosses.store(5, Ordering::Relaxed);
        min_bemf_schedule(&drive);
        assert_eq!(drive.min_bemf_up.load(Ordering::Relaxed), TARGET_MIN_BEMF_COUNTS);
        assert_eq!(drive.min_bemf_down.load(Ordering::Relaxed), TARGET_MIN_BEMF_COUNTS);
    }

    #[test]
    fn store_average_interval_thirds_and_zeroes_nonpositive() {
        let ss = SchedStore::default();
        let sched = ss.sched();
        assert_eq!(store_average_interval(&sched, 900), 300);
        assert_eq!(sched.average_interval.load(Ordering::Relaxed), 300);
        // Non-positive e_com_time → 0.
        assert_eq!(store_average_interval(&sched, 0), 0);
        assert_eq!(store_average_interval(&sched, -6), 0);
        assert_eq!(sched.average_interval.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn filter_and_duty_max_returns_loads_and_maps_filter() {
        let ss = SchedStore::default();
        let ds = DriveStore::default();
        let us = DutyStore::default();
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());

        // zc<100 && ci>500 → filter 12; returns the (running, zc) pair.
        drive.running.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(50, Ordering::Relaxed);
        sched.commutation_interval.store(600, Ordering::Relaxed);
        let (running, zc) = filter_and_duty_max(&sched, &drive, &duty, 100, 300);
        assert!(running);
        assert_eq!(zc, 50);
        assert_eq!(drive.filter_level.load(Ordering::Relaxed), 12);
        // duty_cycle_maximum = am32 low-rpm ceiling of the same inputs.
        assert_eq!(
            duty.duty_cycle_maximum.load(Ordering::Relaxed),
            am32::low_rpm_duty_ceiling(100, true, LOW_RPM_THROTTLE_LIMIT)
        );

        // ci<50 overrides everything → filter 2.
        sched.commutation_interval.store(40, Ordering::Relaxed);
        filter_and_duty_max(&sched, &drive, &duty, 100, 300);
        assert_eq!(drive.filter_level.load(Ordering::Relaxed), 2);

        // Old (zc>=100): the average_interval map branch.
        drive.zero_crosses.store(200, Ordering::Relaxed);
        sched.commutation_interval.store(600, Ordering::Relaxed);
        let (_, zc) = filter_and_duty_max(&sched, &drive, &duty, 100, 300);
        assert_eq!(zc, 200);
        assert_eq!(
            drive.filter_level.load(Ordering::Relaxed) as i32,
            am32::map(300, 100, 500, 3, 12)
        );
    }

    #[test]
    fn bemf_timeout_resets_on_conditions_only() {
        let ds = DriveStore::default();
        let us = DutyStore::default();
        let (drive, duty) = (ds.drive(), us.duty());

        // Neither condition (zc mid, adj high): counter held.
        drive.bemf_timeout_happened.store(7, Ordering::Relaxed);
        duty.adjusted_input.store(500, Ordering::Relaxed);
        bemf_timeout_resets(&drive, &duty, 500);
        assert_eq!(drive.bemf_timeout_happened.load(Ordering::Relaxed), 7);

        // zc > 1000 → cleared.
        bemf_timeout_resets(&drive, &duty, 1001);
        assert_eq!(drive.bemf_timeout_happened.load(Ordering::Relaxed), 0);

        // adj == 0 → cleared.
        drive.bemf_timeout_happened.store(7, Ordering::Relaxed);
        duty.adjusted_input.store(0, Ordering::Relaxed);
        bemf_timeout_resets(&drive, &duty, 50);
        assert_eq!(drive.bemf_timeout_happened.load(Ordering::Relaxed), 0);

        // zc > 100 && adj < 200 → cleared.
        drive.bemf_timeout_happened.store(7, Ordering::Relaxed);
        duty.adjusted_input.store(199, Ordering::Relaxed);
        bemf_timeout_resets(&drive, &duty, 150);
        assert_eq!(drive.bemf_timeout_happened.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn set_input_clamp_startup_window_and_max_cap() {
        let ds = DriveStore::default();
        let us = DutyStore::default();
        let (drive, duty) = (ds.drive(), us.duty());
        duty.duty_cycle_maximum.store(DUTY_FULL, Ordering::Relaxed);

        // Startup window (zc<30): low setpoint clamps UP to MIN_STARTUP_DUTY.
        drive.zero_crosses.store(0, Ordering::Relaxed);
        set_input_clamp(&drive, &duty, 100);
        assert_eq!(duty.duty_cycle_setpoint.load(Ordering::Relaxed), MIN_STARTUP_DUTY);

        // Startup window: full throttle clamps DOWN to STARTUP_MAX_DUTY_CYCLE.
        set_input_clamp(&drive, &duty, 2047);
        assert_eq!(
            duty.duty_cycle_setpoint.load(Ordering::Relaxed),
            STARTUP_MAX_DUTY_CYCLE
        );

        // Out of the startup window (zc>=30): duty_cycle_maximum caps.
        drive.zero_crosses.store(100, Ordering::Relaxed);
        duty.duty_cycle_maximum.store(300, Ordering::Relaxed);
        set_input_clamp(&drive, &duty, 2047);
        assert_eq!(duty.duty_cycle_setpoint.load(Ordering::Relaxed), 300);

        // Disarmed (input<47): setpoint 0, no clamps apply.
        duty.duty_cycle_maximum.store(DUTY_FULL, Ordering::Relaxed);
        set_input_clamp(&drive, &duty, 0);
        assert_eq!(duty.duty_cycle_setpoint.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn uart_deadman_latches_at_limit_and_zeroes_throttle() {
        let us = DutyStore::default();
        let bs = BenchStore::default();
        let (duty, bench) = (us.duty(), bs.bench());

        // Below the limit: counts, throttle untouched.
        duty.uart_duty_input.store(999, Ordering::Relaxed);
        duty.adjusted_input.store(999, Ordering::Relaxed);
        uart_deadman_tick(&duty, &bench);
        assert_eq!(bench.uart_deadman_ticks.load(Ordering::Relaxed), 1);
        assert_eq!(duty.uart_duty_input.load(Ordering::Relaxed), 999);

        // At the limit: next tick latches limit+1 and zeroes throttle.
        bench.uart_deadman_ticks.store(UART_DEADMAN_LIMIT, Ordering::Relaxed);
        uart_deadman_tick(&duty, &bench);
        assert_eq!(bench.uart_deadman_ticks.load(Ordering::Relaxed), UART_DEADMAN_LIMIT + 1);
        assert_eq!(duty.uart_duty_input.load(Ordering::Relaxed), 0);
        assert_eq!(duty.adjusted_input.load(Ordering::Relaxed), 0);

        // Stays latched (does not run past limit+1).
        uart_deadman_tick(&duty, &bench);
        assert_eq!(bench.uart_deadman_ticks.load(Ordering::Relaxed), UART_DEADMAN_LIMIT + 1);
    }

    #[test]
    fn duty_ramp_stores_and_returns_duty_cycle() {
        let ss = SchedStore::default();
        let ds = DriveStore::default();
        let us = DutyStore::default();
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());

        // Startup regime (zc<150): rate 2, so 100 → 102 toward 200.
        drive.zero_crosses.store(0, Ordering::Relaxed);
        duty.last_duty_cycle.store(100, Ordering::Relaxed);
        let v = duty_ramp(&sched, &drive, &duty, 200);
        assert_eq!(v, 102);
        assert_eq!(duty.duty_cycle.load(Ordering::Relaxed), 102);
        assert_eq!(duty.ramp_count.load(Ordering::Relaxed), 1);
        // last_duty_cycle is NOT written here (duty_apply owns that store).
        assert_eq!(duty.last_duty_cycle.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn apply_uart_cmd_per_variant() {
        let us = DutyStore::default();
        let bs = BenchStore::default();
        let (duty, bench) = (us.duty(), bs.bench());

        // Stop: throttle+adjusted+deadman zeroed, stop requested.
        duty.uart_duty_input.store(555, Ordering::Relaxed);
        duty.adjusted_input.store(555, Ordering::Relaxed);
        bench.uart_deadman_ticks.store(10, Ordering::Relaxed);
        apply_uart_cmd(&duty, &bench, Some(UartCmd::Stop));
        assert_eq!(duty.uart_duty_input.load(Ordering::Relaxed), 0);
        assert_eq!(duty.adjusted_input.load(Ordering::Relaxed), 0);
        assert_eq!(bench.uart_deadman_ticks.load(Ordering::Relaxed), 0);
        assert!(bench.stop_req.load(Ordering::Relaxed));

        // SetThrottle mirrors adjusted with the <=48 zero rule.
        bench.uart_deadman_ticks.store(10, Ordering::Relaxed);
        apply_uart_cmd(&duty, &bench, Some(UartCmd::SetThrottle(48)));
        assert_eq!(duty.uart_duty_input.load(Ordering::Relaxed), 48);
        assert_eq!(duty.adjusted_input.load(Ordering::Relaxed), 0);
        assert_eq!(bench.uart_deadman_ticks.load(Ordering::Relaxed), 0);
        apply_uart_cmd(&duty, &bench, Some(UartCmd::SetThrottle(1047)));
        assert_eq!(duty.uart_duty_input.load(Ordering::Relaxed), 1047);
        assert_eq!(duty.adjusted_input.load(Ordering::Relaxed), 1047);

        // TraceToggle flips.
        assert!(!bench.zct_stream_on.load(Ordering::Relaxed));
        apply_uart_cmd(&duty, &bench, Some(UartCmd::TraceToggle));
        assert!(bench.zct_stream_on.load(Ordering::Relaxed));
        apply_uart_cmd(&duty, &bench, Some(UartCmd::TraceToggle));
        assert!(!bench.zct_stream_on.load(Ordering::Relaxed));

        // Info / BbDump / GeckoDump / WaxDump / HistDump set request flags.
        apply_uart_cmd(&duty, &bench, Some(UartCmd::Info));
        assert!(bench.info_req.load(Ordering::Relaxed));
        apply_uart_cmd(&duty, &bench, Some(UartCmd::BbDump));
        assert!(bench.dump_req.load(Ordering::Relaxed));
        apply_uart_cmd(&duty, &bench, Some(UartCmd::GeckoDump));
        assert!(bench.gecko_req.load(Ordering::Relaxed));
        apply_uart_cmd(&duty, &bench, Some(UartCmd::WaxDump));
        assert!(bench.wax_req.load(Ordering::Relaxed));
        apply_uart_cmd(&duty, &bench, Some(UartCmd::HistDump));
        assert!(bench.hist_req.load(Ordering::Relaxed));

        // None: no-op.
        bench.stop_req.store(false, Ordering::Relaxed);
        apply_uart_cmd(&duty, &bench, None);
        assert!(!bench.stop_req.load(Ordering::Relaxed));
        assert_eq!(duty.uart_duty_input.load(Ordering::Relaxed), 1047);
    }

    #[test]
    fn apply_uart_cmd_delay_bumps_and_clamps() {
        let us = DutyStore::default();
        let bs = BenchStore::default();
        let (duty, bench) = (us.duty(), bs.bench());

        // Both start at 0 (inert).
        assert_eq!(bench.delay_in_free.load(Ordering::Relaxed), 0);
        assert_eq!(bench.delay_out_free.load(Ordering::Relaxed), 0);

        // Up bumps by DELAY_BUMP_CYC each press.
        apply_uart_cmd(&duty, &bench, Some(UartCmd::DelayInFreeUp));
        assert_eq!(bench.delay_in_free.load(Ordering::Relaxed), DELAY_BUMP_CYC);
        apply_uart_cmd(&duty, &bench, Some(UartCmd::DelayInFreeUp));
        assert_eq!(bench.delay_in_free.load(Ordering::Relaxed), 2 * DELAY_BUMP_CYC);
        // out-free is an independent cell.
        assert_eq!(bench.delay_out_free.load(Ordering::Relaxed), 0);

        // Down clamps at 0 (never negative / never underflows).
        for _ in 0..10 {
            apply_uart_cmd(&duty, &bench, Some(UartCmd::DelayInFreeDown));
        }
        assert_eq!(bench.delay_in_free.load(Ordering::Relaxed), 0);

        // out-free bumps independently and clamps up at the cap.
        apply_uart_cmd(&duty, &bench, Some(UartCmd::DelayOutFreeUp));
        assert_eq!(bench.delay_out_free.load(Ordering::Relaxed), DELAY_BUMP_CYC);
        assert_eq!(bench.delay_in_free.load(Ordering::Relaxed), 0);
        for _ in 0..1000 {
            apply_uart_cmd(&duty, &bench, Some(UartCmd::DelayOutFreeUp));
        }
        assert_eq!(bench.delay_out_free.load(Ordering::Relaxed), DELAY_CAP_CYC);
        // Down from the cap steps back by exactly one bump.
        apply_uart_cmd(&duty, &bench, Some(UartCmd::DelayOutFreeDown));
        assert_eq!(bench.delay_out_free.load(Ordering::Relaxed), DELAY_CAP_CYC - DELAY_BUMP_CYC);
    }
}
