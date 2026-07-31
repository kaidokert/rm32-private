//! ISR logic functions — shared between MCU targets.
//!
//! Each function contains the actual ISR body. The MCU-specific
//! `#[interrupt]` wrappers in `interrupts_g071.rs` / `interrupts_f051.rs`
//! just clear the flag and call these.

use crate::isr::{self, TargetIsrState};
use crate::mcu::ChipConfig;
use rm32::hal::InputCapture;
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
            #[cfg(not(feature = "benchuart"))]
            state.hal.input.receive_dshot_dma();
            rtt_target::rprintln!("[isr] state moved to ISR_LOCAL, DMA re-armed");
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
#[cfg(all(feature = "benchuart", feature = "stm32l431"))]
pub static EXC_N: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(all(feature = "benchuart", feature = "stm32l431"))]
pub static EXC_MAX: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(all(feature = "benchuart", feature = "stm32l431"))]
pub static EXC_COMMS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Onboard flight recorder (poll-law-safe charts): every 0.5s (10k
/// ticks) the tick ISR samples (ci, current mA, vbat mV) into a RAM
/// ring — zero host involvement during the run; 'B' dumps post-run.
/// 800 slots = the last ~6.7 minutes.
pub const SR_N: usize = 800;
#[cfg(all(feature = "benchuart", feature = "stm32l431"))]
pub static SR_CI: [core::sync::atomic::AtomicU16; SR_N] =
    [const { core::sync::atomic::AtomicU16::new(0) }; SR_N];
#[cfg(all(feature = "benchuart", feature = "stm32l431"))]
pub static SR_MA: [core::sync::atomic::AtomicU16; SR_N] =
    [const { core::sync::atomic::AtomicU16::new(0) }; SR_N];
#[cfg(all(feature = "benchuart", feature = "stm32l431"))]
pub static SR_MV: [core::sync::atomic::AtomicU16; SR_N] =
    [const { core::sync::atomic::AtomicU16::new(0) }; SR_N];
#[cfg(all(feature = "benchuart", feature = "stm32l431"))]
pub static SR_HEAD: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// 20kHz control loop tick (TIM6 ISR body).
pub fn handle_tim6() {
    // Minimal-overhead timing bracket: DWT.CYCCNT delta written to a plain
    // store (single-writer, no fetch_max LDREX/STREX). Liveness counter
    // (dbg_isr_tick_inc) moved AFTER the bracket so it doesn't bias the
    // measurement. Previously: rprintln heartbeat every 20000 ticks +
    // fetch_max CAS + dbg_isr_tick_inc inside the bracket added ~30-50
    // cycles of measurement overhead per tick.
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    let cyc_start = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    let tim6_comms_entry =
        crate::mcu_l431::interrupts::LEAN_COMMS.load(core::sync::atomic::Ordering::Relaxed);

    let state = ISR_LOCAL.get();
    let shared = isr::shared();
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    crate::phase::canary(0); // site 0: tim6 entry

    // Flight recorder: one sample per 0.5s from the tick ISR (constant
    // cost: a modulo check on the tick counter + three stores).
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    {
        use core::sync::atomic::Ordering;
        if shared.dbg_isr_tick() % 10_000 == 0 {
            let h = SR_HEAD.load(Ordering::Relaxed) as usize % SR_N;
            SR_CI[h].store(
                shared.commutation_interval().min(65535) as u16,
                Ordering::Relaxed,
            );
            SR_MA[h].store(shared.actual_current().max(0) as u16, Ordering::Relaxed);
            SR_MV[h].store(shared.battery_voltage(), Ordering::Relaxed);
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

    // Comparator-level history (deaf-hiccup discriminator): one comp
    // read per tick shifted into edge_probe::LEVEL_HIST — constant
    // per-tick cost at prio 3. Decoded per window from the probe row.
    #[cfg(feature = "zctrace")]
    crate::edge_probe::level_tick(!comp_at_pre_zc_level());

    // WAXWING-lite: per-tick phase-voltage ring write (no-op until the
    // injected burst is armed via 'J'). Constant per-tick cost. The comp
    // VALUE bit is captured in the same tick (packed into T1S bit 15) so
    // the frozen deaf window carries arc + comparator level time-aligned.
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    crate::mcu_l431::adc::wax_tick(state.commutation.step(), !comp_at_pre_zc_level());

    // Mid-window N-pin trap (storm hunt): at 20 kHz, if comp drive is on
    // and the motor is in interrupt mode, the current driven phase's N
    // pin must still be AF. A hit here with the post-com_step check
    // clean = a concurrent writer reverts it between commutations.
    #[cfg(all(feature = "zctrace", feature = "stm32l431"))]
    if shared.running() && !shared.old_routine() {
        use core::sync::atomic::Ordering;
        let comp_on = match crate::phase::COMP_PWM_LIVE.load(Ordering::Relaxed) {
            1 => false,
            2 => true,
            _ => state.config.comp_pwm != 0,
        };
        if comp_on {
            let step = state.commutation.step();
            let (on_a, pin) = match step {
                1 | 6 => (false, 1u32),
                4 | 5 => (false, 0),
                2 | 3 => (true, 7),
                _ => (false, 1),
            };
            let moder = unsafe {
                core::ptr::read_volatile(
                    (if on_a { 0x4800_0000u32 } else { 0x4800_0400 }) as *const u32,
                )
            };
            if (moder >> (pin * 2)) & 3 != 0b10 {
                use rm32::hal::IntervalTimer as _;
                crate::edge_probe::midw_violation(step, state.hal.interval.count());
            }
        }
    }
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    {
        let cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
        shared.dbg_tim6_last_cyc_set(cyc_end.wrapping_sub(cyc_start));
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
            crate::bench_hist::record(site, cyc_end.wrapping_sub(cyc_start));
        }
        #[cfg(all(feature = "benchuart", feature = "stm32g431"))]
        crate::bench_hist::record(
            crate::bench_hist::Site::Tim6,
            cyc_end.wrapping_sub(cyc_start),
        );
    }
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    crate::phase::canary(1); // site 1: tim6 exit (ten_khz_tick ran between 0 and 1)
    // 20 kHz gate latch (AM32-verbatim staleness for the COMP gate),
    // CLAMPED to AM32's average_interval domain (main.c clamps
    // average_interval to 5000 at desync/timeout sites — and their COMP
    // gate reads THAT variable). rm32 had split the quantity: main's
    // desync clamp landed on a private copy while the gate read raw
    // e_com/3 — after kick-era garbage intervals entered the ring the
    // gate inflated to multi-ms, one early edge camped for the whole
    // gate at priority 0 (59k re-entries, ~30 ms blackout, fall
    // autopsy 07-25), starved its own accepts, and the timeout/kick
    // churn became self-sustaining. The clamp bounds any camp to
    // <=1.25 ms and reconnects the gate to AM32's guarded domain.
    // NOTE: an earlier continuous min(,5000) clamp here was WRONG — AM32's
    // 5000 is a one-shot desync reset, not a cap; clamping continuously
    // halves the legitimate spin-up gate (avg ~10000 era) and invites
    // early accepts during every climb (4/4 climb failures measured).
    // The 30 ms camp-blackout bound needs a camp-side fix instead.
    // COMP gate stale average — latched every tick in ALL builds (this
    // is control, not instrumentation; see crate::comp_gate).
    crate::comp_gate::latch((shared.e_com_time() / 3).max(0) as u32);
    shared.dbg_isr_tick_inc();
}

/// Commutation timer expired (TIM14 ISR body).
pub fn handle_tim14() {
    let state = ISR_LOCAL.get();
    let shared = isr::shared();
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    let cyc_start = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
    // Edge probe: interval count at TIM16 entry, BEFORE any step logic —
    // vs the scheduled wait+1 this is the commutation fire latency.
    #[cfg(feature = "zctrace")]
    {
        use rm32::hal::IntervalTimer as _;
        crate::edge_probe::tim16_fired(state.hal.interval.count());
    }
    rm32::control::isr_logic::commutation_timer_expired(
        &mut state.commutation,
        &mut state.bemf,
        shared,
        &mut state.hal.com_timer,
        &mut state.hal.comp,
        &mut state.hal.phase,
        state.config.bi_direction != 0,
        state.config.stall_protection != 0 || state.config.rc_car_reverse != 0,
    );
    // Parity re-qual excursion counters (clone metric).
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
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
    // Comp-engagement violation trap (storm hunt): immediately after the
    // commutation's com_step, the driven phase's N pin MUST be AF when
    // complementary drive is active. A violation here = the com_step's
    // own write didn't land; a violation appearing LATER (statistical
    // MODER sampling) = a concurrent writer reverted it.
    #[cfg(all(feature = "zctrace", feature = "stm32l431"))]
    {
        use core::sync::atomic::Ordering;
        let comp_on = match crate::phase::COMP_PWM_LIVE.load(Ordering::Relaxed) {
            1 => false,
            2 => true,
            _ => state.config.comp_pwm != 0,
        };
        if comp_on {
            let step = state.commutation.step();
            // step -> (driven phase idx, N-pin port A?, pin#): A=PB1 B=PB0 C=PA7
            let (idx, on_a, pin) = match step {
                1 | 6 => (0usize, false, 1u32),
                4 | 5 => (1, false, 0),
                2 | 3 => (2, true, 7),
                _ => (0, false, 1),
            };
            let moder = unsafe {
                core::ptr::read_volatile(
                    (if on_a { 0x4800_0000u32 } else { 0x4800_0400 }) as *const u32,
                )
            };
            let ok = (moder >> (pin * 2)) & 3 == 0b10;
            crate::edge_probe::npin_check(idx, ok);
        }
    }
    // Blackbox: one REF per commutation step; data = commutation interval.
    // GATED on the zct stream being armed: the lean-build A/B measured
    // per-commutation instrumentation at priority 0 as a 3x camp-storm
    // amplifier (70 vs 15-32 entries/window). Instruments now cost only
    // while a capture is armed ('Z'), like any attached scope.
    #[cfg(all(
        feature = "blackbox",
        feature = "zctrace",
        any(feature = "stm32l431", feature = "stm32g431")
    ))]
    if crate::bench_zct::enabled() {
        crate::bench_bb::record(
            rm32::blackbox::EV_REF,
            state.commutation.step(),
            shared.commutation_interval().min(u16::MAX as u32) as u16,
        );
    }
    #[cfg(all(
        feature = "blackbox",
        not(feature = "zctrace"),
        any(feature = "stm32l431", feature = "stm32g431")
    ))]
    crate::bench_bb::record(
        rm32::blackbox::EV_REF,
        state.commutation.step(),
        shared.commutation_interval().min(u16::MAX as u32) as u16,
    );
    // NOTE: a freeze-on-fall trigger (4 consecutive early accepts at
    // duty>500 -> bench_zct::freeze()) lived here during the spiral
    // hunt. It caught the onset — root cause was a diagnostic print in
    // the TIM16 ISR delaying commutation (see mcu_l431/interrupts.rs) —
    // and was then removed: always-armed, it silences the trace on the
    // first transient above 50% throttle, blocking envelope capture.
    // bench_zct::freeze() stays available for future one-shot captures.
    // ZC trace: one 15-byte record per commutation (minz wire format),
    // plus the edge-probe companion row on the same gate decision. The
    // probe snapshot RESETS every commutation regardless — window
    // counters must not leak across gated-off stretches.
    #[cfg(feature = "zctrace")]
    {
        let avg = ((shared.e_com_time() / 3).max(0) as u32).min(u16::MAX as u32) as u16;
        let pushed = crate::bench_zct::write(
            state.commutation.step(),
            shared.old_routine(),
            state.bemf.this_zc_time(),
            shared.commutation_interval().min(u16::MAX as u32) as u16,
            state.bemf.wait_time(),
            shared.duty_cycle(),
            shared.dbg_isr_tick() as u16,
            avg,
        );
        let (first_edge, entries, tim16_lat, last_arm, gated_clears, persist_rejects) =
            crate::edge_probe::take();
        if let Some(batching) = pushed {
            // Probe row's 4th u16 slot: per-commutation INJECTED CURRENT
            // (raw counts, hardware-timed mid-PWM-ON) — repurposed from
            // avg, which duplicates the zct row. Decoder: x26.855 mA.
            #[cfg(feature = "benchuart")]
            let cur = crate::mcu_l431::adc::injected_current_raw();
            #[cfg(not(feature = "benchuart"))]
            let cur = avg;
            crate::bench_zct::write_probe(
                batching,
                state.commutation.step(),
                shared.old_routine(),
                first_edge,
                entries,
                tim16_lat,
                cur,
                gated_clears,
                persist_rejects,
                last_arm,
            );
        }
    }
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    {
        let cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
        shared.dbg_tim14_last_cyc_set(cyc_end.wrapping_sub(cyc_start));
        #[cfg(all(
            feature = "benchuart",
            any(feature = "stm32l431", feature = "stm32g431")
        ))]
        crate::bench_hist::record(
            crate::bench_hist::Site::Tim16,
            cyc_end.wrapping_sub(cyc_start),
        );
    }
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    crate::phase::canary(2); // site 2: tim14 exit
}

/// BEMF zero-cross detected (COMP ISR body).
pub fn handle_comp() {
    let state = ISR_LOCAL.get();
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    let (shared, cyc_start) = (isr::shared(), unsafe {
        (*cortex_m::peripheral::DWT::PTR).cyccnt.read()
    });
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
                rm32::blackbox::EV_ACC,
                state.commutation.step(),
                isr::shared().commutation_interval().min(u16::MAX as u32) as u16,
            );
        }
    }
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    {
        let cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
        shared.dbg_comp_last_cyc_set(cyc_end.wrapping_sub(cyc_start));
        #[cfg(all(
            feature = "benchuart",
            any(feature = "stm32l431", feature = "stm32g431")
        ))]
        crate::bench_hist::record(
            crate::bench_hist::Site::Comp,
            cyc_end.wrapping_sub(cyc_start),
        );
    }
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    crate::phase::canary(3); // site 3: comp ISR exit
}

/// COMP gate helper for the L431 wrapper's gate-closed classification:
/// true if the comparator currently sits at the PRE-zero-cross level
/// (`output_level() == rising` — the level the persistence filter
/// rejects). COMP-ISR context only (touches `ISR_LOCAL`).
#[cfg(feature = "stm32l431")]
pub fn comp_at_pre_zc_level() -> bool {
    use rm32::hal::Comparator as _;
    let state = ISR_LOCAL.get();
    state.hal.comp.output_level() == state.commutation.rising()
}

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
                rm32::edt::EdtFrame::Extended(v) => v,
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

    // Debug: emit a sample of the buffer once per ~50 frames
    static mut FRAME_COUNT: u32 = 0;
    let count = unsafe {
        FRAME_COUNT = FRAME_COUNT.wrapping_add(1);
        FRAME_COUNT
    };
    let i_set = shared.input_set();
    let s_pwm = shared.servo_pwm();
    // exti diagnostic — RTT only, not UART (UART log gets drowned otherwise).
    if count % 200 == 1 {
        rtt_target::rprintln!(
            "[exti] frame#{} pin_high={} input_set={} servo_pwm={} buf[0..4]={} {} {} {}",
            count,
            pin_high,
            i_set,
            s_pwm,
            buf[0],
            buf[1],
            buf[2],
            buf[3]
        );
    }

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
                    rtt_target::rprintln!("[exti] DETECTED DShot");
                    shared.set_dshot(true);
                    // Store output prescaler for bidir DShot response timing
                    if let Some(psc) = actions.next_capture.prescaler {
                        state.hal.input.set_output_prescaler(psc);
                    }
                }
                DetectedProtocol::Servo => {
                    rtt_target::rprintln!("[exti] DETECTED Servo PWM");
                    shared.set_servo_pwm(true);
                }
            }
        }
        TransferAction::DshotThrottle { value, telemetry } => {
            // Standard DSHOT throttle is always valid; EDT (Extended DSHOT
            // Telemetry, BF cmd 13) is an *extension*, not a precondition for
            // throttle acceptance. The previous `if edt_armed || value == 0`
            // gate silently dropped every non-zero throttle from vanilla
            // DSHOT300/600 because BF never sends EDT_ENABLE in those modes.
            shared.set_newinput(value);
            // EDT disarm: zero throttle with EDT_ARM_ENABLE clears EDT_ARMED
            if value == 0 && state.edt_arm_enable {
                state.edt_armed = false;
            }
            if telemetry {
                shared.set_send_telemetry(true);
            }
            shared.set_signal_timeout(0);
        }
        TransferAction::DshotCommand { cmd, telemetry } => {
            shared.set_newinput(0);
            if telemetry {
                shared.set_send_telemetry(true);
            }
            shared.set_signal_timeout(0);
            let result = state.cmd.process(
                cmd,
                shared.armed(),
                shared.running(),
                &mut state.config,
                &mut state.forward,
                &mut state.edt_armed,
                state.edt_arm_enable,
            );
            match result {
                rm32::dshot_commands::CommandResult::SaveSettings => {
                    shared.set_save_settings_flag(true);
                }
                rm32::dshot_commands::CommandResult::PlayTone(_tone) => {}
                rm32::dshot_commands::CommandResult::SendEscInfo => {
                    shared.set_send_esc_info_flag(true);
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
            // Persist calibration to EEPROM config; main loop will save to flash
            state.config.servo_low_threshold = low_threshold;
            state.config.servo_high_threshold = high_threshold;
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
        shared.dbg_bidir_evt_inc();
    }
    // Bench-debug counters for bidir-DSHOT investigation.
    // crc_pass: a Throttle/Command return means decode_frame produced a valid
    //   frame (CRC matched, fields parsed). crc_fail: dshot mode active but
    //   action came back None (BadCrc or InvalidTiming inside decode_frame).
    shared.dbg_set_high_pin_n(actions.high_pin_count);
    let crc_pass = matches!(
        actions.action,
        rm32::transfer::TransferAction::DshotThrottle { .. }
            | rm32::transfer::TransferAction::DshotCommand { .. }
    );
    let is_dshot_none =
        matches!(actions.action, rm32::transfer::TransferAction::None) && shared.dshot();
    if crc_pass {
        shared.dbg_crc_pass_inc();
    } else if is_dshot_none {
        shared.dbg_crc_fail_inc();
    }
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
