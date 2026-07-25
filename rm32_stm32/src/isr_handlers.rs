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

    let state = ISR_LOCAL.get();
    let shared = isr::shared();

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

    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    {
        let cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
        shared.dbg_tim6_last_cyc_set(cyc_end.wrapping_sub(cyc_start));
    }
    shared.dbg_isr_tick_inc();
}

/// Commutation timer expired (TIM14 ISR body).
pub fn handle_tim14() {
    let state = ISR_LOCAL.get();
    let shared = isr::shared();
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    let cyc_start = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
    rm32::control::isr_logic::commutation_timer_expired(
        &mut state.commutation,
        &mut state.bemf,
        shared,
        &mut state.hal.com_timer,
        &mut state.hal.comp,
        &mut state.hal.phase,
        state.config.bi_direction != 0,
    );
    // Blackbox: one REF per commutation step; data = commutation interval.
    #[cfg(all(
        feature = "blackbox",
        any(feature = "stm32l431", feature = "stm32g431")
    ))]
    crate::bench_bb::record(
        rm32::blackbox::EV_REF,
        state.commutation.step(),
        shared.commutation_interval().min(u16::MAX as u32) as u16,
    );
    // ZC trace: one 15-byte record per commutation (minz wire format).
    #[cfg(feature = "zctrace")]
    crate::bench_zct::write(
        state.commutation.step(),
        shared.old_routine(),
        state.bemf.this_zc_time(),
        shared.commutation_interval().min(u16::MAX as u32) as u16,
        state.bemf.wait_time(),
        shared.duty_cycle(),
        shared.dbg_isr_tick() as u16,
        ((shared.e_com_time() / 3).max(0) as u32).min(u16::MAX as u32) as u16,
    );
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    {
        let cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
        shared.dbg_tim14_last_cyc_set(cyc_end.wrapping_sub(cyc_start));
    }
}

/// BEMF zero-cross detected (COMP ISR body).
pub fn handle_comp() {
    let state = ISR_LOCAL.get();
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    let (shared, cyc_start) = (isr::shared(), unsafe {
        (*cortex_m::peripheral::DWT::PTR).cyccnt.read()
    });
    rm32::control::isr_logic::bemf_zero_cross(
        &state.commutation,
        &mut state.bemf,
        &mut state.hal.comp,
        &mut state.hal.interval,
        &mut state.hal.com_timer,
    );
    // Blackbox: one ACC per COMP ZC ISR (entry-level granularity for now —
    // acceptance vs persistence-reject isn't distinguished until the core
    // exposes it). data = commutation interval.
    #[cfg(all(
        feature = "blackbox",
        any(feature = "stm32l431", feature = "stm32g431")
    ))]
    crate::bench_bb::record(
        rm32::blackbox::EV_ACC,
        state.commutation.step(),
        isr::shared().commutation_interval().min(u16::MAX as u32) as u16,
    );
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    {
        let cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
        shared.dbg_comp_last_cyc_set(cyc_end.wrapping_sub(cyc_start));
    }
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
