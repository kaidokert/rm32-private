//! ISR logic functions — shared between MCU targets.
//!
//! Each function contains the actual ISR body. The MCU-specific
//! `#[interrupt]` wrappers in `interrupts_g071.rs` / `interrupts_f051.rs`
//! just clear the flag and call these.

use crate::diag::{DIAG, DiagSink, IsrSite};
use crate::isr::{self, TargetIsrState};
use crate::mcu::ChipConfig;
use rm32::hal::{DshotOutputPrescaler, InputCapture};
use rm32::transfer::{DetectedProtocol, TransferAction};

/// Single-core ISR-local cell for zero-overhead mutable ISR state.
///
/// # Safety invariants for `Sync` impl
///
/// This type wraps `UnsafeCell<Option<TargetIsrState>>` and implements `Sync`
/// (required for `static` placement). This is sound because:
///
/// 1. **Single writer**: Only called from ISR handlers that share the same
///    NVIC priority level. Cortex-M's priority-based preemption model
///    guarantees that equal-priority ISRs cannot preempt each other.
///
/// 2. **Single core**: All STM32 targets (G071/F051/L431/G431) are
///    single-core. No other hart can access this cell.
///
/// 3. **No main-loop access**: The main loop communicates via `SharedState`
///    atomics, never touching `ISR_LOCAL`.
///
/// 4. **Init-once**: `get()` lazily initializes from `take_isr_state()`
///    exactly once (first ISR invocation). Subsequent calls return the
///    same `&mut`. The `Option` transitions None→Some exactly once.
///
/// If any of these invariants change (e.g., adding a higher-priority ISR
/// that accesses motor state), this must be replaced with
/// `cortex_m::interrupt::Mutex<RefCell<...>>`.
struct IsrCell(core::cell::UnsafeCell<Option<TargetIsrState>>);

// SAFETY: See struct-level doc. Single-core + same-priority ISR = exclusive access.
unsafe impl Sync for IsrCell {}

impl IsrCell {
    const fn new() -> Self {
        Self(core::cell::UnsafeCell::new(None))
    }

    /// Get or initialize the ISR state.
    /// Panics if state was never initialized — the project-wide panic handler
    /// in `panic.rs` forces all FETs off before halting.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    fn get(&self) -> &mut TargetIsrState {
        // SAFETY: Called only from ISR context at a single priority level.
        // No concurrent access possible (see struct-level safety doc).
        let opt = unsafe { &mut *self.0.get() };
        let needed_init = opt.is_none();
        let state =
            opt.get_or_insert_with(|| isr::take_isr_state().expect("ISR state not initialized"));
        if needed_init {
            // First-time init: state was just moved from ISR_STATE into this
            // ISR_LOCAL cell, so any DMA pointer set up in main against the
            // ISR_STATE address is now stale. Re-arm DMA at the new address.
            // benchuart: PA2 is USART2 RX — DShot capture stays unarmed.
            // (No print here — this runs at ISR priority on first entry.)
            #[cfg(not(feature = "benchuart"))]
            state.hal.input.receive_dshot_dma();
        }
        state
    }
}

static ISR_LOCAL: IsrCell = IsrCell::new();

/// Parity re-qual counters (clone metric): per locked commutation,
/// excursion = |thiszc*1000/avg - 1000| permille vs the rolling
/// average (e_com/3). EXC_N counts >250 permille (the clone's ">25%"
/// bucket), EXC_MAX holds the worst permille, EXC_COMMS counts locked
/// commutations measured. Cumulative; host computes segment deltas.
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
pub static EXC_N: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
pub static EXC_MAX: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
pub static EXC_COMMS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Onboard flight recorder (poll-law-safe charts): every 0.5s (10k
/// ticks) the tick ISR samples (ci, current mA, vbat mV) into a RAM
/// ring — zero host involvement during the run; 'B' dumps post-run.
/// 800 slots = the last ~6.7 minutes.
pub const SR_N: usize = 800;
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
pub static SR_CI: [core::sync::atomic::AtomicU16; SR_N] =
    [const { core::sync::atomic::AtomicU16::new(0) }; SR_N];
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
pub static SR_MA: [core::sync::atomic::AtomicU16; SR_N] =
    [const { core::sync::atomic::AtomicU16::new(0) }; SR_N];
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
pub static SR_MV: [core::sync::atomic::AtomicU16; SR_N] =
    [const { core::sync::atomic::AtomicU16::new(0) }; SR_N];
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
pub static SR_HEAD: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// binz bench: TIM6 handler section costs (max cycles per section within
/// one tick): [entry latency (CNT at entry), ten_khz_tick, tones, tail to
/// the T6 measurement]. Reset by `H`.
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static T6_CP: [core::sync::atomic::AtomicU32; 4] =
    [const { core::sync::atomic::AtomicU32::new(0) }; 4];

#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
static T6_PREV: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
#[inline(always)]
fn t6_cp(i: usize) {
    use core::sync::atomic::Ordering::Relaxed;
    let c = unsafe { (*crate::pac::TIM6::ptr()).cnt().read().bits() };
    let d = if i == 0 {
        c
    } else {
        let p = T6_PREV.load(Relaxed);
        if c >= p { c - p } else { c + 3200 - p } // one wrap at most
    };
    T6_PREV.store(c, Relaxed);
    if d > T6_CP[i].load(Relaxed) {
        T6_CP[i].store(d, Relaxed);
    }
}

/// binz bench: TIM6 tick-to-tick ENTRY gaps, stamped on a free-running
/// SysTick (24-bit down-counter at the CPU clock, no interrupt; armed by
/// `tick_gap_init`). The overrun counter above is censored at one period;
/// this one sees how late a tick really ran and whether one was lost
/// outright (gap >= 2 periods: UIF is a single flag, a second update while
/// it is still set disappears). Model: firmware50's guard `gap_max_us`.
/// Reset by `H` (`tick_gap_reset`).
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static GAP_MAX: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// Gaps over 1.5 periods (4800 cycles): the tick ran at least half late.
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static GAP_LATE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// Gaps of 2 periods or more (6400 cycles): at least one tick lost.
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static GAP_LOST: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// Previous entry stamp; `u32::MAX` = no previous sample (after a reset).
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
static GAP_PREV: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(u32::MAX);

#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub fn tick_gap_init() {
    // SAFETY: SysTick is unused on the G071 path (no SysTick handler, no
    // HAL delay on it); free-running at the core clock, interrupt off.
    unsafe {
        let st = &*cortex_m::peripheral::SYST::PTR;
        st.rvr.write(0x00FF_FFFF);
        st.cvr.write(0);
        st.csr.write(0b101); // CLKSOURCE = core, TICKINT = 0, ENABLE
    }
}

#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub fn tick_gap_reset() {
    use core::sync::atomic::Ordering::Relaxed;
    // The ISR may store GAP_PREV between these; at worst one gap after the
    // reset is measured from a stamp taken just before it (still a real gap).
    GAP_PREV.store(u32::MAX, Relaxed);
    GAP_MAX.store(0, Relaxed);
    GAP_LATE.store(0, Relaxed);
    GAP_LOST.store(0, Relaxed);
}

#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
#[inline(always)]
fn tick_gap() {
    use core::sync::atomic::Ordering::Relaxed;
    // SAFETY: read-only access to the free-running SysTick counter.
    let now = unsafe { (*cortex_m::peripheral::SYST::PTR).cvr.read() };
    let prev = GAP_PREV.load(Relaxed);
    GAP_PREV.store(now, Relaxed);
    if prev == u32::MAX {
        return;
    }
    // Down-counter: elapsed = prev - now, modulo 2^24 (wraps every 262 ms).
    let gap = prev.wrapping_sub(now) & 0x00FF_FFFF;
    if gap > GAP_MAX.load(Relaxed) {
        GAP_MAX.store(gap, Relaxed);
    }
    if gap >= 4800 {
        GAP_LATE.store(GAP_LATE.load(Relaxed).wrapping_add(1), Relaxed);
        if gap >= 6400 {
            GAP_LOST.store(GAP_LOST.load(Relaxed).wrapping_add(1), Relaxed);
        }
    }
}

/// 20kHz control loop tick (TIM6 ISR body).
pub fn handle_tim6() {
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    tick_gap();
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    t6_cp(0);
    // Three-shunt boards: TIM6 TRGO hardware-triggers one ADC scan per tick;
    // fold the scan the previous update produced (first thing, before the
    // new scan's first conversion lands). See mcu_g071::adc::tick_scan.
    #[cfg(all(feature = "stm32g071", rm32_three_shunt))]
    crate::mcu::active::adc::tick_scan();
    // Minimal-overhead timing bracket: DWT.CYCCNT delta written to a plain
    // store (single-writer, no fetch_max LDREX/STREX). Liveness counter
    // (dbg_isr_tick_inc) moved AFTER the bracket so it doesn't bias the
    // measurement. Previously: rprintln heartbeat every 20000 ticks +
    // fetch_max CAS + dbg_isr_tick_inc inside the bracket added ~30-50
    // cycles of measurement overhead per tick.
    let cyc_start = DIAG.stamp();
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    let tim6_comms_entry =
        crate::mcu_l431::interrupts::LEAN_COMMS.load(core::sync::atomic::Ordering::Relaxed);

    let state = ISR_LOCAL.get();
    let shared = isr::shared();

    // Flight recorder: one sample per 0.5s from the tick ISR (constant
    // cost: a modulo check on the tick counter + three stores).
    #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
    {
        use core::sync::atomic::Ordering;
        if isr::tick_count() % 10_000 == 0 {
            let h = SR_HEAD.load(Ordering::Relaxed) as usize % SR_N;
            SR_CI[h].store(
                shared.commutation_interval().min(65535) as u16,
                Ordering::Relaxed,
            );
            SR_MA[h].store(shared.actual_current().max(0) as u16, Ordering::Relaxed);
            SR_MV[h].store(crate::isr::vbat_mv(), Ordering::Relaxed);
            SR_HEAD.store(
                SR_HEAD.load(Ordering::Relaxed).wrapping_add(1),
                Ordering::Relaxed,
            );
        }
    }

    let mut ctx = rm32::control::context::MotorContext {
        commutation: &mut state.commutation,
        bemf: &mut state.bemf,
        duty: &mut state.duty,
        config: &state.config,
        armed_timeout_count: &mut state.armed_timeout_count,
        voltage_based_ramp: state.voltage_based_ramp,
        shared,
        hal: &mut state.hal,
    };
    rm32::control::isr_logic::ten_khz_tick(&mut ctx);
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    t6_cp(1);

    // A3 tone stepper (after the control tick so tone PWM writes land
    // last within the tick). While a tone is active the control path's
    // idle duty writes would mute it — re-assert the tone duty every
    // tick (one HAL write, constant cost). Aborts instantly on running.
    {
        use rm32::bridge::Bridge as _;
        use rm32::hal::{MotorHal as _, PhaseOutput as _};
        use rm32::tone::ToneAction;
        let req = shared.take_tone_request();
        match state
            .tone
            .tick::<crate::platform::StmPlatform>(req, shared.running())
        {
            ToneAction::StartNote(n) => {
                #[cfg(feature = "debuguart")]
                bump_single_writer(&TONE_STARTS);
                let mut bridge = state.hal.bridge();
                bridge.set_period(crate::mcu::Chip::TIM1_AUTORELOAD);
                bridge.set_tone_divider(n.prescaler);
                // AM32 sounds.c setVolume(): CCR = beep_volume(0-11) * 3.
                let vol = (state.config.beep_volume.min(11) as u16) * 3;
                bridge.set_duty_all(vol);
                // The core's drive decision (rm32::bridge::drive_complementary)
                // reduces to the config bit here: StartNote is only returned
                // while the motor is not running, and old_routine is a
                // running mode. AM32's tones likewise follow comp_pwm alone.
                bridge.com_step(n.step, state.config.use_comp_pwm());
            }
            ToneAction::Silence => {
                #[cfg(feature = "debuguart")]
                if shared.running() {
                    bump_single_writer(&TONE_ABORTS);
                } else {
                    bump_single_writer(&TONE_ENDS);
                }
                let mut bridge = state.hal.bridge();
                bridge.all_off();
                bridge.set_tone_divider(0);
                bridge.set_period(crate::mcu::Chip::TIM1_AUTORELOAD);
            }
            ToneAction::Idle => {
                if state.tone.active() {
                    let vol = (state.config.beep_volume.min(11) as u16) * 3;
                    state.hal.bridge().set_duty_all(vol);
                }
            }
        }
    }

    // Comparator-level history (deaf-hiccup discriminator): one comp
    // read per tick shifted into edge_probe::LEVEL_HIST — constant
    // per-tick cost at prio 3. Decoded per window from the probe row.
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    t6_cp(2);
    #[cfg(feature = "zctrace")]
    crate::edge_probe::level_tick(!comp_at_pre_zc_level());

    // WAXWING-lite: per-tick phase-voltage ring write (no-op until the
    // injected burst is armed via 'J'). Constant per-tick cost. The comp
    // VALUE bit is captured in the same tick (packed into T1S bit 15) so
    // the frozen deaf window carries arc + comparator level time-aligned.
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    crate::mcu_l431::adc::wax_tick(state.commutation.step(), !comp_at_pre_zc_level());

    {
        let cycles = DIAG.bracket(IsrSite::Tick, cyc_start);
        // Split by preemption: a commutation firing DURING this tick
        // (LEAN_COMMS advanced) inflates wall-time by ~one TIM16 handler.
        // Clean = control-tick OWN work; preempted = own + commutation.
        #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
        {
            let comms_after =
                crate::mcu_l431::interrupts::LEAN_COMMS.load(core::sync::atomic::Ordering::Relaxed);
            let site = if comms_after != tim6_comms_entry {
                crate::bench_hist::Site::Tim6Pre
            } else {
                crate::bench_hist::Site::Tim6
            };
            crate::bench_hist::record(site, cycles);
        }
        #[cfg(all(feature = "benchuart", feature = "stm32g431"))]
        crate::bench_hist::record(crate::bench_hist::Site::Tim6, cycles);
        let _ = cycles;
    }
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    t6_cp(3);
    // binz bench: measured TIM6 handler wall time — TIM6 counts CPU cycles
    // from its own update (PSC 0, ARR 3199), so CNT at exit is the elapsed
    // time since the tick including entry latency and COM/COMP preemption.
    // UIF already set again at exit = the next tick is already due (overrun).
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    {
        use core::sync::atomic::Ordering::Relaxed;
        let t6 = unsafe { &*crate::pac::TIM6::ptr() };
        let cnt = t6.cnt().read().bits();
        if t6.sr().read().bits() & 1 != 0 {
            T6_OVR.store(T6_OVR.load(Relaxed).wrapping_add(1), Relaxed);
        } else {
            if cnt > T6_MAX.load(Relaxed) {
                T6_MAX.store(cnt, Relaxed);
            }
            T6_SUM.store(T6_SUM.load(Relaxed).wrapping_add(cnt), Relaxed);
            T6_N.store(T6_N.load(Relaxed).wrapping_add(1), Relaxed);
        }
    }
}

/// binz bench: max TIM6 handler wall time (cycles since its update) and
/// overrun count; reset by the `H` hold-start command.
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static T6_MAX: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// binz bench: COMP ISR entries and camp re-entries (gate closed, post-ZC
/// level: pending left set, the ISR re-fires until the gate opens).
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static COMP_ENTRIES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static COMP_CAMPS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// binz bench: sum / count of TIM6 wall time over non-overrun ticks (mean).
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static T6_SUM: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static T6_N: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub static T6_OVR: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Commutation timer expired (TIM14 ISR body).
pub fn handle_tim14() {
    let state = ISR_LOCAL.get();
    let shared = isr::shared();
    DIAG.on_com_step(&mut state.commutation);
    let cyc_start = DIAG.stamp();
    // Edge probe: interval count at TIM16 entry, BEFORE any step logic —
    // vs the scheduled wait+1 this is the commutation fire latency.
    #[cfg(feature = "zctrace")]
    {
        use rm32::hal::IntervalTimer as _;
        crate::edge_probe::tim16_fired(state.hal.interval.count());
    }
    rm32::control::isr_logic::commutation_timer_expired::<crate::platform::StmPlatform, _, _, _, _>(
        &mut state.commutation,
        &mut state.bemf,
        shared,
        &mut state.hal.com_timer,
        &mut state.hal.comp,
        &mut state.hal.phase,
        state.config.use_comp_pwm(),
        state.config.is_bidirectional(),
        state.config.use_strict_changeover(),
    );
    // Parity re-qual excursion counters (clone metric).
    #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
    {
        use core::sync::atomic::Ordering;
        let tz = state.bemf.this_zc_time() as u32;
        // Parity re-qual excursion counters (clone metric) — measured on
        // every LOCKED commutation, whole envelope.
        if shared.running() && !shared.old_routine() && tz > 0 {
            let avg = (shared.e_com_time() / 3).max(1) as u32;
            let permille = if tz >= avg {
                (tz - avg) * 1000 / avg
            } else {
                (avg - tz) * 1000 / avg
            };
            EXC_COMMS.store(
                EXC_COMMS.load(Ordering::Relaxed).wrapping_add(1),
                Ordering::Relaxed,
            );
            if permille > 250 {
                EXC_N.store(
                    EXC_N.load(Ordering::Relaxed).wrapping_add(1),
                    Ordering::Relaxed,
                );
            }
            if permille > EXC_MAX.load(Ordering::Relaxed) {
                EXC_MAX.store(permille, Ordering::Relaxed);
            }
        }
    }
    {
        let cycles = DIAG.bracket(IsrSite::Commutation, cyc_start);
        #[cfg(all(
            feature = "benchuart",
            any(feature = "stm32l431", feature = "stm32g431")
        ))]
        crate::bench_hist::record(crate::bench_hist::Site::Tim16, cycles);
        let _ = cycles;
    }
}

/// BEMF zero-cross detected (COMP ISR body).
pub fn handle_comp() {
    let state = ISR_LOCAL.get();
    let cyc_start = DIAG.stamp();
    let accepted = rm32::control::isr_logic::bemf_zero_cross(
        &state.commutation,
        &mut state.bemf,
        &mut state.hal.comp,
        &mut state.hal.interval,
        &mut state.hal.com_timer,
    );
    #[cfg(feature = "zctrace")]
    if !accepted {
        crate::edge_probe::persist_reject();
    }
    #[cfg(not(feature = "zctrace"))]
    let _ = accepted;
    // binz bench: per-sector interval statistics on every accept. Accepts
    // only happen in interrupt mode (the comparator interrupt is enabled
    // only there — commutation_timer_expired), so no mode check is needed.
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    if accepted {
        crate::sector_hist::accept(state.bemf.this_zc_time(), state.commutation.step());
    }
    // Blackbox: one ACC per genuine acceptance. data = commutation interval.
    // Gated on zct-armed (see the REF record above for the rationale).
    #[cfg(all(
        feature = "blackbox",
        any(feature = "stm32l431", feature = "stm32g431")
    ))]
    if accepted {
        #[cfg(feature = "zctrace")]
        let armed = crate::bench_zct::enabled();
        #[cfg(not(feature = "zctrace"))]
        let armed = true;
        if armed {
            crate::bench_bb::record(
                rm32_bench::blackbox::EV_ACC,
                state.commutation.step(),
                isr::shared().commutation_interval().min(u16::MAX as u32) as u16,
            );
        }
    }
    {
        let cycles = DIAG.bracket(IsrSite::ZeroCross, cyc_start);
        #[cfg(all(
            feature = "benchuart",
            any(feature = "stm32l431", feature = "stm32g431")
        ))]
        crate::bench_hist::record(crate::bench_hist::Site::Comp, cycles);
        let _ = cycles;
    }
}

/// COMP gate helper for the L431 wrapper's gate-closed classification:
/// true if the comparator currently sits at the PRE-zero-cross level
/// (`output_level() == rising` — the level the persistence filter
/// rejects). COMP-ISR context only (touches `ISR_LOCAL`).
#[cfg(any(feature = "stm32l431", feature = "stm32g071"))]
pub fn comp_at_pre_zc_level() -> bool {
    use rm32::hal::BemfSensor as _;
    let state = ISR_LOCAL.get();
    state.hal.comp.level() == state.commutation.rising()
}

/// EDT-debug: typed frames sent + last 12-bit value (heartbeat consumer).
#[cfg(feature = "debuguart")]
pub static EDT_SENT: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);
#[cfg(feature = "debuguart")]
pub static EDT_LAST: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);

/// Single-writer (TIM6 ISR) counter bump: load + store, no RMW — thumbv6m
/// has no atomic fetch_add, and only this ISR ever writes the counters.
#[cfg(feature = "debuguart")]
#[inline(always)]
fn bump_single_writer(c: &core::sync::atomic::AtomicU32) {
    use core::sync::atomic::Ordering::Relaxed;
    c.store(c.load(Relaxed).wrapping_add(1), Relaxed);
}

/// Tone-path decision counters (instrument-decisions-not-outcomes):
/// starts = StartNote actions applied (note transitions), aborts =
/// running-flag kills, ends = sequences completed. Read by the [su]
/// heartbeat; never printed from the ISR.
#[cfg(feature = "debuguart")]
pub static TONE_STARTS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "debuguart")]
pub static TONE_ABORTS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "debuguart")]
pub static TONE_ENDS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Poll-print disturbance A/B (item 7): when set, main deliberately
/// prints while the motor RUNS — the banned observer behavior,
/// reintroduced as a controlled experiment to name the mechanism
/// (RTT critical sections vs PB6-TX->PB7 comparator-input crosstalk).
/// Level-set by DSHOT cmd 42 (ON) / 47 (OFF) at stop — level, not
/// toggle, so BF's command repeats are idempotent.
#[cfg(feature = "debuguart")]
pub static PRINT_BLAST: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// DMA transfer complete (input capture ISR body).
pub fn handle_dma_tc() {
    let state = ISR_LOCAL.get();
    let shared = isr::shared();

    if shared.armed() && shared.dshot_telemetry() {
        if state.hal.input.is_output() {
            state.hal.input.receive_dshot_dma();
        } else {
            let gcr = state.hal.input.gcr_buffer();

            // EDT: decide whether to send eRPM or extended data frame
            let value_12bit = match state.edt.next_frame(
                shared.actual_current(),
                shared.battery_voltage(),
                shared.degrees_celsius(),
            ) {
                rm32::edt::EdtFrame::Extended(v) => {
                    // EDT-debug: count typed frames actually sent + latch
                    // the last value (read by the [edt] heartbeat line).
                    #[cfg(feature = "debuguart")]
                    {
                        use core::sync::atomic::Ordering;
                        EDT_SENT.store(
                            EDT_SENT.load(Ordering::Relaxed).wrapping_add(1),
                            Ordering::Relaxed,
                        );
                        EDT_LAST.store(v, Ordering::Relaxed);
                    }
                    v
                }
                rm32::edt::EdtFrame::Erpm => {
                    rm32::dshot::erpm_to_12bit(shared.e_com_time() as u16, shared.running())
                }
            };
            rm32::dshot::encode_gcr_frame(value_12bit, gcr, 7, crate::mcu::Chip::GCR_SHIFT);

            state.hal.input.send_dshot_dma();
        }
    }
}

/// Software-triggered frame processing (EXTI ISR body).
/// Returns the capture size for the next DMA cycle.
pub fn handle_exti_frame() -> rm32::transfer::CaptureConfig {
    let state = ISR_LOCAL.get();
    let shared = isr::shared();

    let buf = state.hal.input.dma_buffer();

    // Snapshot the first 8 edges for the debug ring buffer; the borrow on
    // `buf` outlives the function and would conflict with later
    // `state.hal.input.set_output_prescaler(...)` mutable borrow. Cheap copy.
    #[cfg(feature = "debuguart")]
    let buf_snap_first8: [u32; 8] = {
        let mut a = [0u32; 8];
        for (i, slot) in a.iter_mut().enumerate() {
            *slot = buf[i];
        }
        a
    };

    let pin_high = state.hal.input.input_pin_state();

    // Frame counter — feeds the frame-history ring only. (The former
    // per-200-frames [exti] rprintln here is gone: no prints in ISRs.)
    #[cfg(feature = "debuguart")]
    let count = {
        static mut FRAME_COUNT: u32 = 0;
        unsafe {
            FRAME_COUNT = FRAME_COUNT.wrapping_add(1);
            FRAME_COUNT
        }
    };

    let mut zic = shared.zero_input_count();
    let actions = state.transfer.process(
        buf,
        shared.input_set(),
        shared.dshot(),
        shared.servo_pwm(),
        shared.dshot_telemetry(),
        shared.armed(),
        pin_high,
        shared.adjusted_input(),
        shared.newinput(),
        state.config.bi_direction != 0,
        state.config.disable_stick_calibration != 0,
        &mut zic,
        state.frametime_low,
        state.frametime_high,
        crate::mcu::Chip::CPU_FREQUENCY_MHZ as u8,
    );
    shared.set_zero_input_count(zic);

    match actions.action {
        TransferAction::InputDetected(proto) => {
            shared.set_input_set(true);
            match proto {
                DetectedProtocol::Dshot => {
                    shared.set_dshot(true);
                    if let Some(prescaler) = actions.next_capture.prescaler {
                        state.hal.input.set_output_prescaler(
                            DshotOutputPrescaler::new(prescaler)
                                .expect("invalid DShot output prescaler"),
                        );
                    }
                }
                DetectedProtocol::Servo => shared.set_servo_pwm(true),
            }
        }
        TransferAction::DshotThrottle { value, telemetry } => {
            if !state.edt_arm_enable || state.edt_armed || value == 0 {
                shared.set_newinput(value);
            }
            if value == 0 && state.edt_arm_enable {
                state.edt_armed = false;
            }
            if telemetry {
                shared.set_send_telemetry(true);
            }
            shared.set_signal_timeout(0);
        }
        TransferAction::DshotCommand { cmd, telemetry } => {
            // Bench experiment lever (cmds 42/47, unassigned in the
            // DSHOT command space): level-set the PB6 print-blast used
            // by the poll-print-disturbance A/B (item 7). Level rather
            // than toggle — BF repeats command frames, and repeated
            // sets must be idempotent. Main blasts while running.
            #[cfg(feature = "debuguart")]
            if cmd == 42 || cmd == 47 {
                use core::sync::atomic::Ordering;
                PRINT_BLAST.store(cmd == 42, Ordering::Relaxed);
            }
            // Command-arrival telemetry (EDT handshake debug): count every
            // decoded command frame and publish the last cmd id. Rides the
            // post-detection-idle dbg fields in the [loop] line
            // (bidir_evt = count, hi_pin_n = last cmd).
            DIAG.dshot_command(cmd as u8);
            shared.set_newinput(0);
            if telemetry {
                shared.set_send_telemetry(true);
            }
            shared.set_signal_timeout(0);
            let old_config = state.config;
            let result = state.cmd.process(
                cmd,
                shared.armed(),
                shared.running(),
                &mut state.config,
                &mut state.forward,
                &mut state.edt_armed,
                state.edt_arm_enable,
            );
            for (offset, (old, new)) in old_config
                .as_bytes()
                .iter()
                .zip(state.config.as_bytes().iter())
                .enumerate()
            {
                if old != new {
                    shared.push_config_write(offset as u8, *new);
                }
            }
            match result {
                rm32::dshot_commands::CommandResult::SaveSettings => {
                    shared.set_save_settings_flag(true);
                }
                rm32::dshot_commands::CommandResult::PlayTone(tone) => {
                    // A3: beacons route through the tick tone stepper.
                    shared.set_tone_request(tone);
                }
                rm32::dshot_commands::CommandResult::SendEscInfo => {
                    shared.set_send_esc_info_flag(true);
                }
                rm32::dshot_commands::CommandResult::ProgrammingCommit { position, value } => {
                    // A4: arbitrary-byte programming writes reach main too.
                    shared.push_config_write(position as u8, value);
                }
                _ => {}
            }
            shared.set_forward(state.forward);
            if state.cmd.take_edt_init() {
                state.edt.request_init();
            }
            if state.cmd.take_edt_deinit() {
                state.edt.request_deinit();
            }
        }
        TransferAction::ServoThrottle(value) => {
            shared.set_newinput(value);
            shared.set_signal_timeout(0);
        }
        TransferAction::ServoCalibrating => {
            shared.set_signal_timeout(0);
        }
        TransferAction::ServoCalibrationDone {
            low_threshold,
            high_threshold,
        } => {
            // Persist calibration to EEPROM config; main loop will save to
            // flash. A4: publish the bytes so main's copy is current when
            // the save flag is acted on.
            state.config.servo_low_threshold = low_threshold;
            state.config.servo_high_threshold = high_threshold;
            shared.push_config_write(
                core::mem::offset_of!(rm32::config::EepromConfig, servo_low_threshold) as u8,
                low_threshold,
            );
            shared.push_config_write(
                core::mem::offset_of!(rm32::config::EepromConfig, servo_high_threshold) as u8,
                high_threshold,
            );
            shared.set_save_settings_flag(true);
            shared.set_signal_timeout(0);
        }
        TransferAction::None => {}
    }
    if let Some((low, high)) = actions.frametime {
        state.frametime_low = low;
        state.frametime_high = high;
    }
    if actions.bidir_detected {
        shared.set_dshot_telemetry(true);
        DIAG.bidir_detected();
        // BENCH: auto-start EDT at bidir commit. Production activation is
        // AM32-verbatim (DSHOT cmd 13, 6x, while armed+stopped) — but BF
        // sends its enable burst at motor-init and at FLIGHT-ARM, and the
        // CLI-driven bench performs neither while the ESC can decode
        // (measured: cmd-frame counter stayed 0 across FC reboots; the
        // boot-time burst lands in our detection window). Auto-init here
        // exercises the full EDT frame path against BF's parser.
        #[cfg(feature = "debuguart")]
        state.edt.request_init();
    }
    // Bench-debug counters for bidir-DSHOT investigation.
    // crc_pass: a Throttle/Command return means decode_frame produced a valid
    //   frame (CRC matched, fields parsed). crc_fail: dshot mode active but
    //   action came back None (BadCrc or InvalidTiming inside decode_frame).
    let crc_pass = matches!(
        actions.action,
        rm32::transfer::TransferAction::DshotThrottle { .. }
            | rm32::transfer::TransferAction::DshotCommand { .. }
    );
    DIAG.input_frame(&actions, crc_pass, || shared.dshot());
    // Push frame snapshot to ring buffer for main-loop dump (debuguart only).
    // Only push when dshot mode is set, otherwise the buffer fills with pre-
    // detection garbage. Cheap critical section, ~52 bytes copied.
    #[cfg(feature = "debuguart")]
    if shared.dshot() {
        crate::dbg_frame_history::push(crate::dbg_frame_history::FrameSnap {
            n: count,
            buf: buf_snap_first8,
            crc_pass,
            bidir: shared.dshot_telemetry(),
        });
    }

    actions.next_capture
}

/// CRSF UART RX byte handler. Call from UART RX interrupt with each received byte.
pub fn handle_crsf_byte(byte: u8) {
    let state = ISR_LOCAL.get();
    let shared = isr::shared();

    if let Some(rm32::crsf::CrsfResult::Channels(channels)) = state.crsf.feed(byte) {
        let throttle =
            rm32::crsf::CrsfParser::channel_to_throttle(channels[rm32::crsf::THROTTLE_CHANNEL]);
        shared.set_newinput(throttle);
        shared.set_signal_timeout(0);
        if !shared.input_set() {
            shared.set_input_set(true);
        }
    }
}
