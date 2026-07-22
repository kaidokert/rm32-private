//! AM32-L431 control-loop clone (operator directive 2026-07-20:
//! "cut ruthlessly and build up — literally do what AM32 does in its
//! 3 control loops").
//!
//! CONTROL is a line-cited transliteration of AM32 `Src/main.c` +
//! `Mcu/l431/Src/{stm32l4xx_it.c,comparator.c,peripherals.c}` for
//! target VIMDRONES_L431, factory-default EEPROM, with the fork's
//! `UART_DUTY_MODE` + `ZC_TRACE` bench protocols. Every control-path
//! constant / branch cites its AM32 line.
//!
//! The four AM32 contexts, mapped 1:1 to hardware here:
//!   1. COMP ISR  (their COMP_IRQHandler + interruptRoutine)  prio 0
//!      — gate on INTERVAL_TIMER, persistence, mask, timestamp, arm COM.
//!   2. COM ISR   (TIM1_UP_TIM16 vector -> PeriodElapsedCallback) prio 0
//!      — commutate, interval blend, advance, re-enable comp.
//!   3. TIM6 20 kHz (their tenKhzRoutine)  prio 3
//!      — duty pipeline + apply, polling-mode (old_routine) commutation.
//!   4. main while(1) — uart poll, e_com_time / average_interval,
//!      setInput duty setpoint, variable_pwm, desync_check, bemf
//!      timeout, filter_level / duty_cycle_maximum maps, telemetry.
//!
//! Quantities are AM32's: INTERVAL_TIMER (TIM2) + COM_TIMER (TIM16)
//! both tick at **0.5 µs** (PSC=39 @ 80 MHz) — so `commutation_interval`,
//! `average_interval`, `waitTime`, `thiszctime` are all 0.5 µs units,
//! exactly as in AM32.
//!
//! NON-CONTROL additions (observer-only, allowed by the directive):
//!   - ZC_TRACE 15-byte record stream (5B A9 sync) — AM32 fork verbatim.
//!   - 64-event black box (minz_core::blackbox), dumped on kill / `b`.
//!   - injected ADC (A/B/current/vbat) harvested in TIM6 for telemetry
//!     and the two bench-safety kills ONLY.
//!   - three bench-safety KILLS: hard overcurrent, absolute vbat floor,
//!     IWDG. None modulate the loop; they only stop it.
//!
//! Comparator-polarity note (load-bearing): minz's `comp2::value()` is
//! the INVERSE of AM32's `getCompOutputLevel()` (POLARITY wiring — see
//! minz/CLAUDE.md and `AM32 main.c:831 "polarity reversed"`). So AM32's
//! `getCompOutputLevel() == rising` (reject) transliterates to
//! `comp2::value() != rising` here, and the EXTI edge for each sector
//! uses the minz-verified `drive::edges_for(3, sector)` (equivalent to
//! AM32's `changeCompInput` `if(rising)` edge select in this polarity).

#![no_std]
#![no_main]

use core::cell::RefCell;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};

use cortex_m::interrupt::{Mutex, free};
use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;

use minz::adc_sync;
use minz::am32_timers::{
    com_clear_flag, com_set_arr, com_timer_init, disable_com_timer_int, interval_cnt,
    interval_timer_init, set_and_enable_com_int, set_interval_cnt,
};
use minz::board_init::{BoardInit, configure_motor_pwm_pins, init};
use minz::comp2;
use minz::current_adc::SenseAdc;
use minz::hal::pac::interrupt;
use minz::hal::prelude::*;
use minz::hal::serial::{Config, Serial};
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::priority;
use minz::tim1_motor_pwm::{self, max_duty};
use minz::uart_tx::{TX_RING_LEN, UartTxWriter};

use minz_core::am32::{self, RxRing, UartDuty, ZCT_REC, ZctRing};
use minz_core::am32_loop::{
    BEMF_TIMEOUT_TICKS, Bench, DUTY_FULL, Drive, Duty, INIT_INTERVAL_TICKS, MIN_STARTUP_DUTY,
    POLLING_MODE_CHANGEOVER, STARTUP_INTERVAL_TICKS, Sched, TARGET_MIN_BEMF_COUNTS, TEMP_ADVANCE,
    VARIABLE_PWM, apply_uart_cmd, bemf_timeout_resets, duty_ramp, filter_and_duty_max,
    min_bemf_schedule, set_input_clamp, store_average_interval, uart_deadman_tick,
};
use minz_core::blackbox::{self, BlackBox, EV_ACC, EV_DSY, EV_REF, Event};

use rtt_target::rprintln;

// ===============================================================
// Bench-only constants. The AM32 factory constants (DUTY_FULL,
// LOOP_FREQUENCY_HZ, TARGET_MIN_BEMF_COUNTS, BAD_COUNT_THRESHOLD,
// POLLING_MODE_CHANGEOVER, TEMP_ADVANCE, the interval seeds, the
// duty-pipeline defaults, VARIABLE_PWM, BEMF_TIMEOUT_TICKS,
// UART_DEADMAN_LIMIT) moved to `minz_core::am32_loop` with their
// AM32 citations; the ones this file still names are re-imported.
// ===============================================================

/// TIM1 base ARR = 24 kHz carrier. AM32 targets.h:5335
/// `TIM1_AUTORELOAD = CPU_FREQUENCY_MHZ*1e6/NOMINAL_PWM - 1 = 3332`.
const TIMER1_MAX_ARR: u16 = minz::TIM1_AUTORELOAD; // 3332

/// 2 Mbaud link (both directions) — the minz-rig baud (AM32 fork
/// uart_duty_init main.c:1363, motor_tester2 parity).
const BAUD: u32 = 2_000_000;

// --- Bench-safety kill thresholds (observer ADC; kills only) ---
/// ~85 ms current average over raw injected ch8 counts.
const OC_KILL_RAW_AVG: u32 = 205;
const OC_WINDOW_TICKS: u32 = 1700; // ≈ 85 ms at 20 kHz
/// absolute vbat floor, raw injected ch11 counts (~5.95 V).
const VBAT_ABS_FLOOR_RAW: u16 = 793;

// ===============================================================
// AM32 globals as atomics (main.c:400-576). All 0.5 µs / 2000-domain.
// ===============================================================

static COMMUTATION_INTERVAL: AtomicU32 = AtomicU32::new(INIT_INTERVAL_TICKS);
/// The last-6-intervals history (main.c:441,887). e_com_time is their sum.
static COMMUTATION_INTERVALS: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];
static AVERAGE_INTERVAL: AtomicU32 = AtomicU32::new(0);
static LAST_AVERAGE_INTERVAL: AtomicU32 = AtomicU32::new(0);
static LAST_ZC: AtomicU16 = AtomicU16::new(0); // lastzctime
static THIS_ZC: AtomicU16 = AtomicU16::new(0); // thiszctime
static WAIT_TIME: AtomicU16 = AtomicU16::new(0);
static ZERO_CROSSES: AtomicU32 = AtomicU32::new(0);
static BEMF_COUNTER: AtomicU16 = AtomicU16::new(0);
static BAD_COUNT: AtomicU16 = AtomicU16::new(0);
static CURRENT_STEP: AtomicU16 = AtomicU16::new(1); // AM32 step 1..6
static RISING: AtomicBool = AtomicBool::new(true);
static OLD_ROUTINE: AtomicBool = AtomicBool::new(true); // polling at boot
static RUNNING: AtomicBool = AtomicBool::new(false);
static ZCFOUND: AtomicBool = AtomicBool::new(false);
static DESYNC_CHECK: AtomicBool = AtomicBool::new(false);
static DESYNC_HAPPENED: AtomicU32 = AtomicU32::new(0);
static BEMF_TIMEOUT_HAPPENED: AtomicU32 = AtomicU32::new(0);

static FILTER_LEVEL: AtomicU16 = AtomicU16::new(5); // main.c:477
static MIN_BEMF_UP: AtomicU16 = AtomicU16::new(TARGET_MIN_BEMF_COUNTS);
static MIN_BEMF_DOWN: AtomicU16 = AtomicU16::new(TARGET_MIN_BEMF_COUNTS);

static INPUT: AtomicU16 = AtomicU16::new(0); // 0..2047 (main.c:557)
static ADJUSTED_INPUT: AtomicU16 = AtomicU16::new(0);
static UART_DUTY_INPUT: AtomicU16 = AtomicU16::new(0);
static UART_DEADMAN_TICKS: AtomicU32 = AtomicU32::new(0);

static DUTY_CYCLE_SETPOINT: AtomicU16 = AtomicU16::new(0);
static DUTY_CYCLE: AtomicU16 = AtomicU16::new(0);
static LAST_DUTY_CYCLE: AtomicU16 = AtomicU16::new(0);
static DUTY_CYCLE_MAXIMUM: AtomicU16 = AtomicU16::new(DUTY_FULL);
static TENKHZ_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Latched fatal kill (bench-safety). Main prints and holds off.
static KILLED: AtomicBool = AtomicBool::new(false);
static KILL_REASON: AtomicU16 = AtomicU16::new(0); // 1=OC, 2=vbat
static I_RAW: AtomicU16 = AtomicU16::new(0);
static VBAT_RAW: AtomicU16 = AtomicU16::new(0);

/// Stop request from the RX parser (bench pct=0 / 'w'): honored in main.
static STOP_REQ: AtomicBool = AtomicBool::new(false);
static DUMP_REQ: AtomicBool = AtomicBool::new(false);
static INFO_REQ: AtomicBool = AtomicBool::new(false);
static ZCT_STREAM_ON: AtomicBool = AtomicBool::new(true);

/// Which phase floats in each sector (minz textbook convention, matches
/// `set_roles_for_step`): sector 0/3 → C, 1/4 → B, 2/5 → A.
// (SECTOR_FLOAT_PHASE moved to comp2::AM32_SECTOR_FLOAT_PHASE with
// the comparator.c helpers; now_10us moved to minz::am32_timers.)

// ===============================================================
// Black box (observer). minz_core ring behind a critical-section lock.
// ===============================================================
static BB: Mutex<RefCell<BlackBox>> = Mutex::new(RefCell::new(BlackBox::new()));

#[inline]
fn bb_record(ty: u8, sector: u8, data: u16) {
    let t = now_10us();
    free(|cs| BB.borrow(cs).borrow_mut().record(Event { t, ty, sector, data }));
}

// ===============================================================
// INTERVAL_TIMER = TIM2, COM_TIMER = TIM16 — the register wrappers
// (interval_timer_init / interval_cnt / set_interval_cnt /
// com_timer_init / set_and_enable_com_int / disable_com_timer_int /
// com_set_arr / com_clear_flag) live in `minz::am32_timers`.
// ===============================================================

// ===============================================================
// comparator.c transliteration — moved to minz::comp2 (the am32_*
// helpers + AM32_SECTOR_FLOAT_PHASE live with their peripheral).
// Aliased here to keep call sites reading like the AM32 source.
// ===============================================================
use minz::am32_timers::now_10us;
use minz::comp2::{
    am32_change_comp_input as change_comp_input,
    am32_enable_comp_interrupts as enable_comp_interrupts,
    am32_get_bemf_state as get_bemf_state,
    am32_mask_phase_interrupts as mask_phase_interrupts,
};

// ===============================================================
// map()/getAbsDif() (functions.c) now live in `minz_core::am32`.
// ===============================================================

// ===============================================================
// ZC_TRACE ring — 15-byte records (main.c:1536-1567), 5B A9 sync.
// Producer: COM ISR (PeriodElapsedCallback) + polling zcfoundroutine.
// Consumer: main loop → USART1 DMA writer. Guarded with `free` because
// two ISR contexts (prio 0 COM, prio 3 TIM6) can both push.
// ===============================================================
const ZCT_N: usize = 32;
static ZCT_RING: [[AtomicU16; ZCT_REC]; ZCT_N] =
    [const { [const { AtomicU16::new(0) }; ZCT_REC] }; ZCT_N];
static ZCT_HEAD: AtomicUsize = AtomicUsize::new(0);
static ZCT_TAIL: AtomicUsize = AtomicUsize::new(0);
static ZCT_DROP: AtomicU32 = AtomicU32::new(0);

//// BATCH DECIMATION (operator directive 2026-07-20): above the wire's
/// bandwidth the trace switches to 50-commutations-on / 50-off batch
/// mode instead of losing records to saturation aliasing. Batches
/// (not 1-in-N) preserve CONSECUTIVE records so rolling-mean
/// excursion metrics stay valid inside each batch (~44 usable
/// samples per 50). Budget: 2 Mbaud ≈ 13.3k records/s; full rate
/// fits down to ci ≈ 150 ticks (75 µs); batching engages below
/// ci = 200 ticks (100 µs → 10k/s full → 5k/s batched, comfortable).
/// Mode is re-evaluated only at batch boundaries (no mid-batch
/// flapping); flag bit6 marks batched-mode records so the host can
/// segment (decoders mask bits0-2|7 — bit6 is backward-compatible).
static ZCT_COMM_N: AtomicU32 = AtomicU32::new(0);
static ZCT_BATCHING: AtomicBool = AtomicBool::new(false);

// ===============================================================
// USART2 RX byte ring (ISR producer, main consumer).
// ===============================================================
const RX_N: usize = 256;
static RX_RING: [AtomicU16; RX_N] = [const { AtomicU16::new(0) }; RX_N];
static RX_HEAD: AtomicUsize = AtomicUsize::new(0);
static RX_TAIL: AtomicUsize = AtomicUsize::new(0);

/// The coupled RX-ring state, grouped as `minz_core::am32::RxRing`
/// (push/pop host-tested there) and serviced by
/// `minz::usart2_rx::service_rx` — this file owns only the storage
/// and the wiring.
static RX: RxRing<'static, RX_N> = RxRing {
    ring: &RX_RING,
    head: &RX_HEAD,
    tail: &RX_TAIL,
};

// ===============================================================
// Cohesion clusters — each struct bundles the loose statics above
// that are written/read together, holding ONLY `&'static Atomic*`
// refs (storage is unchanged). Every servicing fn takes the clusters
// it touches as `&` params; the ONLY places that name the SCHED /
// DRIVE / DUTY / BENCH / ZCT instances are the ISR trampolines,
// main_entry's top (local wiring), and these definitions.
// (Sched / Drive / Duty / Bench struct types + the pure band fns
// moved to minz_core::am32_loop, host-tested there; ZctTrace stays —
// its impl needs cortex_m `free` + UartTxWriter.)
// ===============================================================

/// ZC_TRACE ring cluster — 15-byte records + batch-decimation state.
struct ZctTrace<'a> {
    /// Ring mechanics live host-tested in `minz_core::am32::ZctRing`;
    /// this composes it with the batch-decimation state.
    ring: ZctRing<'a, ZCT_N>,
    comm_n: &'a AtomicU32,
    batching: &'a AtomicBool,
}

static SCHED: Sched<'static> = Sched {
    commutation_interval: &COMMUTATION_INTERVAL,
    interval_hist: &COMMUTATION_INTERVALS,
    average_interval: &AVERAGE_INTERVAL,
    last_average_interval: &LAST_AVERAGE_INTERVAL,
    last_zc: &LAST_ZC,
    this_zc: &THIS_ZC,
    wait_time: &WAIT_TIME,
};

static DRIVE: Drive<'static> = Drive {
    current_step: &CURRENT_STEP,
    rising: &RISING,
    old_routine: &OLD_ROUTINE,
    running: &RUNNING,
    zcfound: &ZCFOUND,
    bemf_counter: &BEMF_COUNTER,
    min_bemf_up: &MIN_BEMF_UP,
    min_bemf_down: &MIN_BEMF_DOWN,
    zero_crosses: &ZERO_CROSSES,
    filter_level: &FILTER_LEVEL,
    bad_count: &BAD_COUNT,
    desync_check: &DESYNC_CHECK,
    desync_happened: &DESYNC_HAPPENED,
    bemf_timeout_happened: &BEMF_TIMEOUT_HAPPENED,
    tenkhz_counter: &TENKHZ_COUNTER,
    zcfr_guard_hits: &ZCFR_GUARD_HITS,
};

static DUTY: Duty<'static> = Duty {
    input: &INPUT,
    adjusted_input: &ADJUSTED_INPUT,
    uart_duty_input: &UART_DUTY_INPUT,
    duty_cycle_setpoint: &DUTY_CYCLE_SETPOINT,
    duty_cycle: &DUTY_CYCLE,
    last_duty_cycle: &LAST_DUTY_CYCLE,
    duty_cycle_maximum: &DUTY_CYCLE_MAXIMUM,
    ramp_count: &RAMP_COUNT,
    killed: &KILLED,
    kill_reason: &KILL_REASON,
};

static BENCH: Bench<'static> = Bench {
    uart_deadman_ticks: &UART_DEADMAN_TICKS,
    i_raw: &I_RAW,
    vbat_raw: &VBAT_RAW,
    oc_acc: &OC_ACC,
    oc_cnt: &OC_CNT,
    stop_req: &STOP_REQ,
    dump_req: &DUMP_REQ,
    info_req: &INFO_REQ,
    zct_stream_on: &ZCT_STREAM_ON,
};

static ZCT: ZctTrace<'static> = ZctTrace {
    ring: ZctRing {
        ring: &ZCT_RING,
        head: &ZCT_HEAD,
        tail: &ZCT_TAIL,
        drop: &ZCT_DROP,
    },
    comm_n: &ZCT_COMM_N,
    batching: &ZCT_BATCHING,
};

impl ZctTrace<'_> {
    // zct_write (main.c:1542-1567): one canonical row per commutation.
    #[inline]
    fn write(&self, sched: &Sched, drive: &Drive, duty: &Duty) {
        // Batch-decimation gate (am32::zct_batch_gate). Single CI snapshot
        // used for both the gate and the packed record (the value is stable
        // within the calling ISR — this was two separate loads before).
        let n = self.comm_n.fetch_add(1, Ordering::Relaxed);
        let ci_ticks = sched.commutation_interval.load(Ordering::Relaxed);
        let (record, batching) =
            am32::zct_batch_gate(n, ci_ticks, self.batching.load(Ordering::Relaxed));
        self.batching.store(batching, Ordering::Relaxed);
        if !record {
            return; // the skipped half-duty of the batch cycle
        }
        let rec: [u8; ZCT_REC] = am32::zct_pack(
            drive.current_step.load(Ordering::Relaxed) as u8,
            drive.old_routine.load(Ordering::Relaxed),
            batching,
            sched.this_zc.load(Ordering::Relaxed),
            ci_ticks as u16,
            sched.wait_time.load(Ordering::Relaxed),
            duty.duty_cycle.load(Ordering::Relaxed),
            drive.tenkhz_counter.load(Ordering::Relaxed),
            sched.average_interval.load(Ordering::Relaxed) as u16,
        );
        // Dual-producer guard (prio-0 COM + prio-3 TIM6): the core
        // ring is not self-synchronizing; the critical section stays
        // with this platform-side caller.
        free(|_| self.ring.push_rec(&rec));
    }

    /// Drain up to 3 ZC_TRACE records into the USART1 DMA ring (main.c:2347-2357).
    #[inline]
    fn drain(&self, tx: &mut UartTxWriter) {
        self.ring.drain(3, |b| {
            let _ = tx.push(b);
        });
    }
}

// ===============================================================
// commutate() — main.c:854-894 (forward-only factory path).
// ===============================================================
fn commutate(sched: &Sched, drive: &Drive) {
    // step++ ; if step>6 { step=1; desync_check=1 }  (main.c:856-861)
    let mut step = drive.current_step.load(Ordering::Relaxed);
    step += 1;
    if step > 6 {
        step = 1;
        drive.desync_check.store(true, Ordering::Relaxed);
    }
    // rising = step % 2  (main.c:862)
    let rising = (step & 1) == 1;
    drive.rising.store(rising, Ordering::Relaxed);
    drive.current_step.store(step, Ordering::Relaxed);
    let sector = (step - 1) as usize;

    // comStep(step) (main.c:876) — minz role-only flip; CCRs hold the
    // tick-shaped duty. AM32 wraps this in __disable_irq; set_roles_for_step
    // does its own interrupt::free.
    tim1_motor_pwm::set_roles_for_step(sector as u8);
    // changeCompInput() (main.c:879).
    change_comp_input(sector);

    // if average_interval > polling_mode_changeover+500 → old_routine=1
    // (main.c:881-883).
    if sched.average_interval.load(Ordering::Relaxed) > POLLING_MODE_CHANGEOVER + 500 {
        drive.old_routine.store(true, Ordering::Relaxed);
    }
    // bemfcounter=0; zcfound=0 (main.c:885-886).
    drive.bemf_counter.store(0, Ordering::Relaxed);
    drive.zcfound.store(false, Ordering::Relaxed);
    // commutation_intervals[step-1] = commutation_interval (main.c:887).
    sched.intervals().push(sector, sched.commutation_interval.load(Ordering::Relaxed));

    bb_record(EV_REF, sector as u8, sched.commutation_interval.load(Ordering::Relaxed) as u16);
}

// ===============================================================
// getBemfState() (main.c:817-852) — moved to
// minz::comp2::am32_get_bemf_state (it reads comp2::value(), so it
// lives with the peripheral); aliased above as `get_bemf_state`.
// ===============================================================

// ===============================================================
// zcfoundroutine() — main.c:1868-1915 (polling mode, blocking).
// ===============================================================
static ZCFR_GUARD_HITS: AtomicU32 = AtomicU32::new(0);

/// zcfoundroutine blend band (main.c:1870-1874): capture thiszctime,
/// reset INTERVAL_TIMER, blend commutation_interval, derive advance/waitTime.
#[inline]
fn zcfr_blend(sched: &Sched) -> (u32, u32) {
    // thiszctime = INTERVAL_TIMER_COUNT; SET_INTERVAL_TIMER_COUNT(0)
    let thiszc = interval_cnt() as u16; // main.c:1870
    set_interval_cnt(0); // main.c:1871
    sched.this_zc.store(thiszc, Ordering::Relaxed);
    // commutation_interval = (thiszctime + 3*ci)/4  (main.c:1872)
    let ci_old = sched.commutation_interval.load(Ordering::Relaxed);
    let ci = am32::polling_blend(thiszc as u32, ci_old);
    sched.commutation_interval.store(ci, Ordering::Relaxed);
    // advance = temp_advance*ci >> 6 ; waitTime = ci/2 - advance  (1873-4)
    let advance = am32::advance_of(ci, TEMP_ADVANCE);
    let wait = am32::wait_time(ci, advance);
    sched.wait_time.store(wait as u16, Ordering::Relaxed);
    (ci, wait)
}

/// zcfoundroutine spin-wait band (main.c:1875-1879):
/// `while INTERVAL_TIMER_COUNT < waitTime { if zero_crosses<5 break }`.
/// DEVIATION #1: bounded by a spin guard — AM32's loop is unbounded (a
/// wedged INTERVAL_TIMER hangs the 20 kHz ISR forever; AM32 accepts that,
/// the IWDG would reboot). We add a loud 65535-iteration break so the tick
/// can't be captured indefinitely.
#[inline]
fn zcfr_spin_wait(drive: &Drive, zc: u32, wait: u32) {
    let mut guard: u32 = 0;
    loop {
        if interval_cnt() >= wait {
            break;
        }
        if zc < 5 {
            break;
        }
        guard += 1;
        if guard > 65_535 {
            drive.zcfr_guard_hits.fetch_add(1, Ordering::Relaxed);
            break;
        }
    }
}

fn zcfoundroutine(sched: &Sched, drive: &Drive, zct: &ZctTrace, duty: &Duty) {
    let (ci, wait) = zcfr_blend(sched); // main.c:1870-1874
    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    zcfr_spin_wait(drive, zc, wait); // main.c:1875-1879

    com_set_arr(wait as u16); // COM_TIMER->ARR = waitTime (main.c:1884)
    commutate(sched, drive); // main.c:1889
    zct.write(sched, drive, duty); // ZC_TRACE main.c:1891
    drive.bemf_counter.store(0, Ordering::Relaxed); // main.c:1893
    drive.bad_count.store(0, Ordering::Relaxed); // main.c:1894
    drive.zero_crosses.store(zc.saturating_add(1), Ordering::Relaxed); // main.c:1896

    // changeover to interrupt mode (non-stall/non-rc_car path,
    // main.c:1908-1913): commutation_interval < polling_mode_changeover.
    if ci < POLLING_MODE_CHANGEOVER {
        drive.old_routine.store(false, Ordering::Relaxed);
        enable_comp_interrupts();
    }
}

// ===============================================================
// startMotor() — main.c:950-959. Per the directive, comp interrupts are
// NOT enabled here (polling reads level, no EXTI) — the enable happens
// at the polling→interrupt changeover in zcfoundroutine. This is the one
// intentional divergence from AM32's line 958 `enableCompInterrupts()`.
// ===============================================================
fn start_motor(sched: &Sched, drive: &Drive) {
    if !drive.running.load(Ordering::Relaxed) {
        commutate(sched, drive); // main.c:953
        sched.commutation_interval.store(STARTUP_INTERVAL_TICKS, Ordering::Relaxed); // :954
        set_interval_cnt(5000); // SET_INTERVAL_TIMER_COUNT(5000) main.c:955
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.bemf_counter.store(0, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed); // main.c:956
    }
}

// ===============================================================
// Bench-safety kill: float all legs, latch, mask comp, freeze bb.
// ===============================================================
fn safety_kill(drive: &Drive, duty: &Duty, reason: u16) {
    tim1_motor_pwm::all_off();
    drive.running.store(false, Ordering::Relaxed);
    duty.duty_cycle_setpoint.store(0, Ordering::Relaxed);
    duty.duty_cycle.store(0, Ordering::Relaxed);
    duty.last_duty_cycle.store(0, Ordering::Relaxed);
    drive.old_routine.store(true, Ordering::Relaxed);
    mask_phase_interrupts();
    disable_com_timer_int();
    duty.kill_reason.store(reason, Ordering::Relaxed);
    duty.killed.store(true, Ordering::Relaxed);
    free(|cs| BB.borrow(cs).borrow_mut().freeze());
}

// ===============================================================
fn main_entry(tx_writer: &mut UartTxWriter) -> ! {
    // Wire the cluster locals once (the only main-context place allowed to
    // name the static instances). Everything below references these.
    let sched = &SCHED;
    let drive = &DRIVE;
    let duty = &DUTY;
    let bench = &BENCH;
    let zct = &ZCT;
    let rx = &RX;

    // Main-context UART parser state (mirrors uart_duty_poll main.c:1367).
    let mut uart = UartDuty::new();
    // ramp_count local-ish (ramp_divider=0 → ramp every tenKhz tick, so
    // main only needs the maps below; ramp lives in the TIM6 ISR).

    loop {
        minz::iwdg::refresh();
        rx_drain(rx, &mut uart, duty, bench);
        honor_stop(drive, duty, bench);

        // e_com_time (main.c:2159): (sum+4)>>1, 0.5 µs units. Threaded into
        // the average_interval + low-rpm ceiling bands below.
        let e_com_time = sched.intervals().e_com_time();
        // input = uart_duty_get()  (main.c:1131) then setInput()
        set_input(sched, drive, duty);
        min_bemf_schedule(drive);
        variable_pwm_ride(sched);

        let average_interval = store_average_interval(sched, e_com_time);
        desync_check_band(sched, drive, duty, average_interval);

        let (running, zc) = filter_and_duty_max(sched, drive, duty, e_com_time, average_interval);
        bemf_timeout_resets(drive, duty, zc);
        bemf_timeout_rekick(sched, drive, duty, zct, running);

        telemetry_drain(bench, zct, tx_writer);
        handle_requests(sched, drive, duty, bench, zct, tx_writer);
    }
}

/// RX drain band — drain USART2 RX ring → uart_duty parser dispatch
/// (uart_duty_poll main.c:1367).
#[inline]
fn rx_drain(rx: &RxRing<RX_N>, uart: &mut UartDuty, duty: &Duty, bench: &Bench) {
    while let Some(c) = rx.pop() {
        apply_uart_cmd(duty, bench, uart.step(c));
    }
}

/// Bench stop-request band: float, disarm, mask, zero the pipeline.
#[inline]
fn honor_stop(drive: &Drive, duty: &Duty, bench: &Bench) {
    if bench.stop_req.swap(false, Ordering::Relaxed) {
        tim1_motor_pwm::all_off();
        drive.running.store(false, Ordering::Relaxed);
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(0, Ordering::Relaxed);
        duty.duty_cycle_setpoint.store(0, Ordering::Relaxed);
        duty.duty_cycle.store(0, Ordering::Relaxed);
        duty.last_duty_cycle.store(0, Ordering::Relaxed);
        mask_phase_interrupts();
        disable_com_timer_int();
    }
}

// (min_bemf_schedule band moved to minz_core::am32_loop.)

/// variable_pwm carrier-ride band (main.c:2192-2195). mode 1: carrier
/// rides the commutation interval; duty ratio is preserved by the tenKhz
/// `duty*tim1_arr/2000` rescale.
#[inline]
fn variable_pwm_ride(sched: &Sched) {
    if VARIABLE_PWM == 1 {
        let ci = sched.commutation_interval.load(Ordering::Relaxed) as i32;
        let arr = am32::map(ci, 96, 200, (TIMER1_MAX_ARR / 2) as i32, TIMER1_MAX_ARR as i32);
        tim1_motor_pwm::set_carrier_arr(arr as u16);
    }
}

// (store_average_interval band moved to minz_core::am32_loop.)

/// desync_check band (main.c:2284-2300) — non-bi-dir subset.
#[inline]
fn desync_check_band(sched: &Sched, drive: &Drive, duty: &Duty, average_interval: u32) {
    if drive.desync_check.load(Ordering::Relaxed) && drive.zero_crosses.load(Ordering::Relaxed) > 10 {
        let lai = sched.last_average_interval.load(Ordering::Relaxed);
        if am32::desync_due(lai, average_interval) {
            drive.zero_crosses.store(0, Ordering::Relaxed); // main.c:2286
            drive.desync_happened.fetch_add(1, Ordering::Relaxed); // :2287
            // (!bi_direction && input>47) || commutation_interval>1000 → running=0
            let input = duty.input.load(Ordering::Relaxed);
            let ci = sched.commutation_interval.load(Ordering::Relaxed);
            if input > 47 || ci > 1000 {
                drive.running.store(false, Ordering::Relaxed);
            }
            drive.old_routine.store(true, Ordering::Relaxed); // :2291
            duty.last_duty_cycle.store(MIN_STARTUP_DUTY / 2, Ordering::Relaxed); // :2295
            bb_record(EV_DSY, (drive.current_step.load(Ordering::Relaxed) - 1) as u8, average_interval as u16);
        }
        drive.desync_check.store(false, Ordering::Relaxed); // :2297
        sched.last_average_interval.store(average_interval, Ordering::Relaxed); // :2299
    }
}

// (filter_and_duty_max + bemf_timeout_resets bands moved to
// minz_core::am32_loop.)

/// bemf-timeout re-kick band (main.c:2495-2509).
#[inline]
fn bemf_timeout_rekick(sched: &Sched, drive: &Drive, duty: &Duty, zct: &ZctTrace, running: bool) {
    if interval_cnt() > BEMF_TIMEOUT_TICKS && running {
        drive.bemf_timeout_happened.fetch_add(1, Ordering::Relaxed);
        mask_phase_interrupts();
        drive.old_routine.store(true, Ordering::Relaxed);
        if duty.input.load(Ordering::Relaxed) < 48 {
            drive.running.store(false, Ordering::Relaxed);
            sched.commutation_interval.store(5000, Ordering::Relaxed);
        }
        drive.zero_crosses.store(0, Ordering::Relaxed);
        zcfoundroutine(sched, drive, zct, duty);
    }
}

/// telemetry band: drain ZC_TRACE ring → USART1 DMA writer, then service.
#[inline]
fn telemetry_drain(bench: &Bench, zct: &ZctTrace, tx: &mut UartTxWriter) {
    if bench.zct_stream_on.load(Ordering::Relaxed) {
        zct.drain(tx);
    }
    tx.service();
}

/// on-demand info / bb dump / kill-notice band.
#[inline]
fn handle_requests(sched: &Sched, drive: &Drive, duty: &Duty, bench: &Bench, zct: &ZctTrace, tx: &mut UartTxWriter) {
    if bench.info_req.swap(false, Ordering::Relaxed) {
        print_info(sched, drive, duty, bench, zct, tx);
    }
    if bench.dump_req.swap(false, Ordering::Relaxed) {
        dump_bb(tx);
    }
    if duty.killed.swap(false, Ordering::Relaxed) {
        let reason = duty.kill_reason.load(Ordering::Relaxed);
        let _ = write!(
            tx,
            "!! KILL reason={} (1=OC 2=vbat) iraw={} vbat={}\r\n",
            reason,
            bench.i_raw.load(Ordering::Relaxed),
            bench.vbat_raw.load(Ordering::Relaxed),
        );
        dump_bb(tx);
    }
}

/// setInput() duty-setpoint block — main.c:1180-1326 factory subset
/// (armed always true; no sine, no brake, no current limit).
fn set_input(sched: &Sched, drive: &Drive, duty: &Duty) {
    // input = uart_duty_get()  (main.c:1131,1423-1431).
    let input = duty.uart_duty_input.load(Ordering::Relaxed);
    duty.input.store(input, Ordering::Relaxed);
    set_input_arming(sched, drive, duty, input);
    set_input_clamp(drive, duty, input);
}

/// setInput arm/disarm band (main.c:1182-1300, comp_pwm subset).
#[inline]
fn set_input_arming(sched: &Sched, drive: &Drive, duty: &Duty, input: u16) {
    let running = drive.running.load(Ordering::Relaxed);
    if input >= 47 {
        // main.c:1182-1196
        if !running {
            tim1_motor_pwm::all_off(); // main.c:1184
            if !drive.old_routine.load(Ordering::Relaxed) {
                start_motor(sched, drive); // main.c:1185-1187
            }
            drive.running.store(true, Ordering::Relaxed); // main.c:1188
            duty.last_duty_cycle.store(MIN_STARTUP_DUTY, Ordering::Relaxed); // :1189
        }
    } else {
        // input < 47 (main.c:1203-1300, comp_pwm subset)
        if !running {
            drive.old_routine.store(true, Ordering::Relaxed);
            drive.zero_crosses.store(0, Ordering::Relaxed);
            drive.bad_count.store(0, Ordering::Relaxed);
            tim1_motor_pwm::all_off();
        }
    }
}

// (set_input_clamp + apply_uart_cmd bands moved to
// minz_core::am32_loop.)

fn print_info(sched: &Sched, drive: &Drive, duty: &Duty, bench: &Bench, zct: &ZctTrace, tx: &mut UartTxWriter) {
    let _ = write!(
        tx,
        "i step={} old={} run={} ci={} avg={} zc={} duty={} iraw={} vbat={} drop={} guard={}\r\n",
        drive.current_step.load(Ordering::Relaxed),
        drive.old_routine.load(Ordering::Relaxed) as u8,
        drive.running.load(Ordering::Relaxed) as u8,
        sched.commutation_interval.load(Ordering::Relaxed),
        sched.average_interval.load(Ordering::Relaxed),
        drive.zero_crosses.load(Ordering::Relaxed),
        duty.duty_cycle.load(Ordering::Relaxed),
        bench.i_raw.load(Ordering::Relaxed),
        bench.vbat_raw.load(Ordering::Relaxed),
        zct.ring.drop.load(Ordering::Relaxed),
        drive.zcfr_guard_hits.load(Ordering::Relaxed),
    );
}

fn dump_bb(tx: &mut UartTxWriter) {
    free(|cs| {
        let bb = BB.borrow(cs).borrow();
        blackbox::format_dump(bb.replay().copied(), |b: &[u8]| {
            for &byte in b {
                let _ = tx.push(byte);
            }
        });
    });
}

// ===============================================================
#[entry]
fn main() -> ! {
    static mut TX_RING: [u8; TX_RING_LEN] = [0; TX_RING_LEN];

    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();
    let BoardInit {
        mut cp,
        clocks,
        mut ahb2,
        mut apb2,
        mut ccipr,
        ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);

    rprintln!("am32_clone: sysclk={} pclk1={}", clocks.sysclk().raw(), clocks.pclk1().raw());

    // DWT cycle counter for observer timestamps (no periodic ISR).
    let _syst = cp.SYST;
    cp.DCB.enable_trace();
    cp.DWT.enable_cycle_counter();
    unsafe { core::ptr::write_volatile(0xE000_1004 as *mut u32, 0) };

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut gpiob = dp.GPIOB.split(&mut ahb2);

    // USART2 RX on PA2 via CR2.SWAP: AF7 open-drain + pull-up.
    let mut rx2 = gpioa
        .pa2
        .into_alternate_open_drain::<7>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);
    rx2.internal_pull_up(&mut gpioa.pupdr, true);

    // ADC sense pins: PA3 (IN8 current), PA6 (IN11 vbat).
    let pa3 = gpioa.pa3.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa6 = gpioa.pa6.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let _sense = SenseAdc::new(dp.ADC1, dp.ADC_COMMON, pa3, pa6, &mut ahb2, &mut ccipr, clocks);

    // Motor PWM pins → TIM1 AF1.
    configure_motor_pwm_pins(
        gpioa.pa7,
        gpioa.pa8,
        gpioa.pa9,
        gpioa.pa10,
        gpiob.pb0,
        gpiob.pb1,
        &mut gpioa.moder,
        &mut gpioa.otyper,
        &mut gpioa.afrl,
        &mut gpioa.afrh,
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
    );

    // USART1 TX (PB6, half-duplex init then push-pull) — the trace/telemetry link.
    let mut usart_tx = gpiob
        .pb6
        .into_alternate_open_drain::<7>(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    usart_tx.internal_pull_up(&mut gpiob.pupdr, true);
    let serial = Serial::usart1(dp.USART1, (usart_tx,), Config::default().baudrate(BAUD.bps()), clocks, &mut apb2);
    let (mut tx, _) = serial.split();
    minz::uart_tx::usart1_tx_push_pull_pb6();

    // COMP2 BEMF sense.
    let pb4 = gpiob.pb4.into_analog(&mut gpiob.moder, &mut gpiob.pupdr);
    let pb7 = gpiob.pb7.into_analog(&mut gpiob.moder, &mut gpiob.pupdr);
    let pa5 = gpioa.pa5.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa4 = gpioa.pa4.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    comp2::init(pb4, pb7, pa5, pa4, &mut apb2);
    comp2::configure_exti_both_edges();

    // TIM1 motor PWM. TIM1.UIE stays OFF (AM32 zero-CPU-at-carrier); the
    // TIM1_UP_TIM16 vector belongs solely to TIM16 (the COM timer).
    tim1_motor_pwm::init(dp.TIM1, &mut apb2);

    // Injected ADC (A/B/current/vbat) via TIM1 TRGO2 — observer only.
    adc_sync::start(adc_sync::SAMPLE_TICKS);

    // AM32 timers: INTERVAL_TIMER=TIM2, COM_TIMER=TIM16, tenKhz=TIM6.
    interval_timer_init();
    com_timer_init();
    minz::tim6_loop::init();

    // IWDG — the guard that survives the MCU (bench-safety kill #3).
    minz::iwdg::start_1s();

    // USART2 receiver (RXNE IRQ enqueues bytes).
    minz::usart2_rx::init_pa2_rx(dp.USART2, clocks.pclk1().raw(), BAUD);

    // Boot: energize step (AM32 init `comStep(2)` main.c:2087) at duty 0
    // so the polling startup has a driven pair + a matching comp mux, but
    // no current (duty 0 = low-side hold). CURRENT_STEP stays 1.
    tim1_motor_pwm::set_roles_for_step(1);
    change_comp_input(1);
    tim1_motor_pwm::set_duty(0);
    comp2::set_exti_enabled(false); // masked until interrupt mode engages

    writeln!(&mut tx, "\r\n=== am32_clone (UART_DUTY_MODE + ZC_TRACE, {} baud) ===\r", BAUD).ok();
    writeln!(&mut tx, "throttle: '<pct>\\n' (0..100) | 's'/'w' stop | 'Z' trace | 'i' info | 'b' bb\r").ok();

    let mut tx_writer = UartTxWriter::new(tx, TX_RING);

    // Priorities: COMP=0, TIM1_UP_TIM16(=COM)=0, TIM6=3, USART2=2
    // (peripherals.c:450,491 + directive). <<4 IPR encoding via priority.rs.
    unsafe {
        priority::set_prigroup_preempt4_sub0();
        priority::set_irq_prio(Interrupt::COMP, 0);
        priority::set_irq_prio(Interrupt::TIM1_UP_TIM16, 0);
        priority::set_irq_prio(Interrupt::TIM6_DACUNDER, 3);
        priority::set_irq_prio(Interrupt::USART2, 2);
        NVIC::unmask(Interrupt::COMP);
        NVIC::unmask(Interrupt::TIM1_UP_TIM16);
        NVIC::unmask(Interrupt::TIM6_DACUNDER);
        NVIC::unmask(Interrupt::USART2);
    }

    // panic::ensure_rtt() (in init) left PRIMASK=1 — re-enable IRQs.
    unsafe { cortex_m::interrupt::enable() };

    main_entry(&mut tx_writer)
}

// ===============================================================
// COMP ISR — stm32l4xx_it.c:276-290 + interruptRoutine main.c:918-948.
// Priority 0. (Trampoline in the vector table at file end.)
// ===============================================================
#[inline]
fn comp_isr(sched: &Sched, drive: &Drive) {
    let exti = unsafe { &*stm32::EXTI::ptr() };
    if exti.pr1.read().pr22().bit_is_set() {
        // if INTERVAL_TIMER->CNT > average_interval>>1  (it.c:280)
        if interval_cnt() > (sched.average_interval.load(Ordering::Relaxed) >> 1) {
            comp2::clear_pending(); // it.c:281
            interrupt_routine(sched, drive); // it.c:282
        } else {
            // gate closed: clear ONLY if the level sits at the pre-ZC
            // level (AM32: getCompOutputLevel()==rising; minz-inverted →
            // comp2::value() != rising). Else LEAVE PENDING (their camp:
            // a post-ZC crossing re-fires until the gate opens).
            if comp2::value() != drive.rising.load(Ordering::Relaxed) {
                comp2::clear_pending(); // it.c:284-285
            }
        }
    }
}

/// interruptRoutine — main.c:918-948.
#[inline]
fn interrupt_routine(sched: &Sched, drive: &Drive) {
    // persistence: reject while the level is still pre-ZC (main.c:932-940;
    // `getCompOutputLevel()==rising` → return, inverted to `value != rising`).
    let filter = drive.filter_level.load(Ordering::Relaxed);
    let rising = drive.rising.load(Ordering::Relaxed);
    for _ in 0..filter {
        if comp2::value() != rising {
            return;
        }
    }
    free(|_| {
        mask_phase_interrupts(); // main.c:942
        sched.last_zc.store(sched.this_zc.load(Ordering::Relaxed), Ordering::Relaxed); // :943
        let t = interval_cnt() as u16; // :944 thiszctime = INTERVAL_TIMER_COUNT
        sched.this_zc.store(t, Ordering::Relaxed);
        set_interval_cnt(0); // :945
        set_and_enable_com_int(sched.wait_time.load(Ordering::Relaxed).wrapping_add(1)); // :946
    });
    bb_record(EV_ACC, (drive.current_step.load(Ordering::Relaxed) - 1) as u8, sched.this_zc.load(Ordering::Relaxed));
}

// ===============================================================
// COM ISR (TIM16 wrap on the shared TIM1_UP_TIM16 vector) —
// PeriodElapsedCallback main.c:896-916. Priority 0. (Trampoline in
// the vector table at file end.)
// ===============================================================
#[inline]
fn tim1_up_tim16_isr(sched: &Sched, drive: &Drive, zct: &ZctTrace, duty: &Duty) {
    com_clear_flag(); // ack TIM16 UIF (TIM1.UIE is off, so this is the COM tick)
    disable_com_timer_int(); // main.c:898
    commutate(sched, drive); // :899
    // commutation_interval = (ci + (lastzctime+thiszctime)/2) / 2  (:900)
    let ci_old = sched.commutation_interval.load(Ordering::Relaxed);
    let lz = sched.last_zc.load(Ordering::Relaxed) as u32;
    let tz = sched.this_zc.load(Ordering::Relaxed) as u32;
    let ci = am32::blend_interval(ci_old, lz, tz);
    sched.commutation_interval.store(ci, Ordering::Relaxed);
    // advance = ci*temp_advance>>6 ; waitTime = ci/2 - advance  (:902-906)
    let advance = am32::advance_of(ci, TEMP_ADVANCE);
    let wait = am32::wait_time(ci, advance);
    sched.wait_time.store(wait as u16, Ordering::Relaxed);
    zct.write(sched, drive, duty); // ZC_TRACE main.c:908
    if !drive.old_routine.load(Ordering::Relaxed) {
        enable_comp_interrupts(); // main.c:910-912
    }
    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    if zc < 10000 {
        drive.zero_crosses.store(zc + 1, Ordering::Relaxed); // main.c:913-915
    }
}

// ===============================================================
// TIM6 ISR — tenKhzRoutine main.c:1608-1809. Priority 3.
// ===============================================================
static RAMP_COUNT: AtomicU16 = AtomicU16::new(0);
static OC_ACC: AtomicU32 = AtomicU32::new(0);
static OC_CNT: AtomicU32 = AtomicU32::new(0);

// (Trampoline in the vector table at file end.)
#[inline]
fn tim6_dacunder_isr(sched: &Sched, drive: &Drive, duty: &Duty, bench: &Bench, zct: &ZctTrace) {
    minz::tim6_loop::clear_flag();

    // duty_cycle = duty_cycle_setpoint (main.c:1611); tenkhzcounter++ (:1612)
    let setpoint = duty.duty_cycle_setpoint.load(Ordering::Relaxed) as i32;
    drive.tenkhz_counter.store(drive.tenkhz_counter.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);

    if !duty.killed.load(Ordering::Relaxed) {
        let duty_val = duty_ramp(sched, drive, duty, setpoint);
        let running = duty_apply(drive, duty, duty_val);
        polling_bemf_check(sched, drive, duty, zct, running);
    }

    uart_deadman_tick(duty, bench);
    adc_harvest_and_safety(drive, duty, bench);
}

// (duty_ramp band moved to minz_core::am32_loop.)

/// Duty apply band (main.c:1771-1791): CCR write path. Returns the
/// single `running` load so the caller's polling band reuses it
/// (preserves the original one-load ordering).
#[inline]
fn duty_apply(drive: &Drive, duty: &Duty, duty_val: u16) -> bool {
    let tim1_arr = max_duty() as u32;
    let running = drive.running.load(Ordering::Relaxed);
    let input = duty.input.load(Ordering::Relaxed);
    let base = (duty_val as u32 * tim1_arr) / 2000;
    let adjusted = if running && input > 47 { base + 1 } else { base };
    duty.last_duty_cycle.store(duty_val, Ordering::Relaxed); // main.c:1789
    tim1_motor_pwm::set_carrier_arr(tim1_arr as u16); // SET_AUTO_RELOAD_PWM (:1790)
    tim1_motor_pwm::set_duty(adjusted as u16); // SET_DUTY_CYCLE_ALL (:1791)
    running
}

/// old_routine polling band (main.c:1679-1696): comparator BEMF
/// counting toward min_bemf_counts, then the polling ZC accept.
#[inline]
fn polling_bemf_check(sched: &Sched, drive: &Drive, duty: &Duty, zct: &ZctTrace, running: bool) {
    if drive.old_routine.load(Ordering::Relaxed) && running {
        mask_phase_interrupts(); // main.c:1681
        get_bemf_state(drive); // :1682
        if !drive.zcfound.load(Ordering::Relaxed) {
            let rising = drive.rising.load(Ordering::Relaxed);
            let bc = drive.bemf_counter.load(Ordering::Relaxed);
            let thresh = if rising {
                drive.min_bemf_up.load(Ordering::Relaxed)
            } else {
                drive.min_bemf_down.load(Ordering::Relaxed)
            };
            if bc > thresh {
                drive.zcfound.store(true, Ordering::Relaxed);
                zcfoundroutine(sched, drive, zct, duty);
            }
        }
    }
}

// (uart_deadman_tick band moved to minz_core::am32_loop.)

/// Observer ADC harvest + the two bench-safety KILLS (non-AM32;
/// they only stop the loop, never modulate it).
#[inline]
fn adc_harvest_and_safety(drive: &Drive, duty: &Duty, bench: &Bench) {
    let (_a, _b, cur, vbat) = adc_sync::inj_read();
    bench.i_raw.store(cur, Ordering::Relaxed);
    bench.vbat_raw.store(vbat, Ordering::Relaxed);
    let (acc, cnt, tripped) = am32::oc_step(
        bench.oc_acc.load(Ordering::Relaxed),
        bench.oc_cnt.load(Ordering::Relaxed),
        cur,
        OC_WINDOW_TICKS,
        OC_KILL_RAW_AVG,
    );
    bench.oc_acc.store(acc, Ordering::Relaxed);
    bench.oc_cnt.store(cnt, Ordering::Relaxed);
    if tripped {
        safety_kill(drive, duty, 1);
    }
    if vbat < VBAT_ABS_FLOOR_RAW && drive.running.load(Ordering::Relaxed) {
        safety_kill(drive, duty, 2);
    }
}

// ===============================================================
// THE VECTOR TABLE — every #[interrupt] trampoline, together at the
// file's end. Trampolines are the ONLY functions that name the
// static instances; each servicing fn above takes its world as
// parameters.
// ===============================================================

/// COMP (priority 0) — the ZC chain.
#[interrupt]
fn COMP() {
    comp_isr(&SCHED, &DRIVE)
}

/// TIM16 wrap on the shared vector (priority 0) — the COM tick.
#[interrupt]
fn TIM1_UP_TIM16() {
    tim1_up_tim16_isr(&SCHED, &DRIVE, &ZCT, &DUTY)
}

/// TIM6 19.6 kHz (priority 3) — tenKhzRoutine.
#[interrupt]
fn TIM6_DACUNDER() {
    tim6_dacunder_isr(&SCHED, &DRIVE, &DUTY, &BENCH, &ZCT)
}

/// USART2 RX (priority 2) — enqueue bytes; parser runs in main.
/// Body lives with its peripheral: minz::usart2_rx::service_rx
/// (drain RXNE + error clears, AM32 main.c:1417-1419).
#[interrupt]
fn USART2() {
    minz::usart2_rx::service_rx(&RX)
}
