//! RM32 ESC firmware entry point — MCU-independent.
//!
//! All MCU-specific init is in `init::init()`.
//! This file only uses shared types and the `init::InitResult`.

#![no_std]
#![no_main]

use cortex_m_rt::entry;

use rm32::commutation::Commutation;
use rm32::config::EepromConfig;
use rm32::control::state::{BemfState, DutyState};
use rm32::hal::{System, TelemetryUart as _};
use rm32::ws2812::LedStatus;

use rm32::main_state::MainState;
use rm32_stm32::init::InitResult;
use rm32_stm32::isr::{self, IsrState};
use rm32_stm32::mcu::FlashStorage;
use rm32_stm32::mcu::{Chip, ChipConfig};

use rm32::hal::Flash as _;
use rm32::hal::InputCapture;
use rm32::hal::PwmOutput;
use rm32::sounds::Sounds;

// Board configuration generated from YAML by build.rs.
// Override with: BOARD=boards/my_board.yaml cargo build
include!(concat!(env!("OUT_DIR"), "/board_config.rs"));

// Panic handler in rm32_stm32::panic — forces all FETs off before halting.
// Replaces panic_halt which halts without safing hardware.

#[entry]
fn main() -> ! {
    // Cortex-M PRIMASK is 0 after reset (IRQs enabled). Disable until ISR state
    // is installed and the explicit enable below at the bottom of main.
    cortex_m::interrupt::disable();

    // Bench-debug short-circuit: drop into AM32-matching register init and
    // spin. Never returns. See mcu::bringup module for context.
    #[cfg(feature = "bringup")]
    rm32_stm32::mcu::bringup::run_and_spin();

    // Enable DWT cycle counter for lockup detection in main-loop log.
    // Direct register access (DEMCR.TRCENA bit 24 + DWT.CTRL.CYCCNTENA bit 0)
    // — avoids fighting cortex_m::Peripherals::take() which init::init also
    // calls. CYCCNT then auto-increments at SYSCLK rate, wrapping every ~53 s
    // at 80 MHz / u32. M0/M0+ (G071, F051) have no DWT cycle counter — skip.
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    unsafe {
        let demcr = &(*cortex_m::peripheral::DCB::PTR).demcr;
        demcr.write(demcr.read() | (1 << 24));
        let ctrl = &(*cortex_m::peripheral::DWT::PTR).ctrl;
        ctrl.write(ctrl.read() | 1);
    }

    rtt_target::rtt_init_print!();
    #[cfg(feature = "debuguart")]
    rm32_stm32::debug_uart::init();

    // Snapshot reset-cause flags before anything clears them. Sticky in
    // RCC_CSR (or its analog) across resets; AM32 bootloader doesn't clear,
    // so we see exactly why this boot happened. Implementation lives in
    // `mcu::read_and_clear_reset_cause()` per-MCU — main.rs stays portable.
    let reset_cause = rm32_stm32::mcu::read_and_clear_reset_cause();
    rm32_stm32::dprintln!("[rm32] last reset:");
    for label in reset_cause.iter_labels() {
        rm32_stm32::dprintln!("[rm32]   - {}", label);
    }
    if reset_cause.is_empty() {
        rm32_stm32::dprintln!("[rm32]   - (no flags — already cleared)");
    }
    rm32_stm32::dprintln!("[rm32] boot");

    // Drain UART shift register before init::init() reconfigures the clock
    // tree — otherwise an in-flight byte ships at the wrong baud and the next
    // line shows up as garbage on the receiver.
    #[cfg(feature = "debuguart")]
    rm32_stm32::debug_uart::flush();

    // --- MCU-specific init (clocks, GPIO, peripherals, NVIC) ---
    let InitResult {
        mut hal,
        mut sys,
        mut adc,
        mut telem,
    } = rm32_stm32::init::init(BOARD.dead_time, BOARD.bemf_pins);

    // Re-arm USART1 after the clock tree shuffle (and in case any peripheral
    // init touched USART1 even with telem disabled). Cheap, idempotent.
    #[cfg(feature = "debuguart")]
    rm32_stm32::debug_uart::init();
    rm32_stm32::dprintln!("[rm32] init done");

    // --- WS2812 LED: boot indicator (dim red) ---
    let led_pin = rm32_stm32::ws2812_hal::GpioBPin::new(BOARD.led_pin.unwrap_or(8));
    let mut led = rm32_stm32::ws2812_hal::Ws2812Gpio::new(led_pin, Chip::CPU_FREQUENCY_MHZ);
    if BOARD.has_led {
        led.set_status(LedStatus::Boot);
    }
    rm32_stm32::dprintln!("[rm32] led done");

    // --- Startup tune (before peripherals move to ISR) ---
    if BOARD.bridge_enable {
        hal.phase = rm32_stm32::phase::G0APhaseDriver::new_bridge(false);
    }
    {
        let sounds = Sounds::new(Chip::TIM1_AUTORELOAD);
        sounds.play_startup(&mut hal.pwm, &mut hal.phase, &mut sys);
    }
    rm32_stm32::dprintln!("[rm32] tone done");

    // --- RPM pulse output (debug): configure GPIO before phase moves to ISR ---
    if BOARD.pulse_output {
        hal.phase
            .enable_pulse_output::<rm32_stm32::gpio_pin::PB10>();
    }

    // --- Start IWDG watchdog (after startup tune, matching C sequencing) ---
    // ON unconditionally (rung 0 safety directive): a brown-out that wedges
    // the core with the bridge frozen is the burnt-motor scenario — the IWDG
    // is the only layer below firmware that releases it. L431: LSI/16,
    // reload 4000 → 2.0 s (AM32 value). Reloaded once per main-loop pass.
    // NOTE: debug-halting the core >2 s now causes an IWDG reset. That is
    // intentional — do NOT freeze IWDG via DBGMCU: a halt with the motor
    // spinning must not keep the bridge frozen.
    sys.start_watchdog(Chip::WDG_PRESCALER, Chip::WDG_RELOAD);
    rm32_stm32::dprintln!("[rm32] wdg ON (2s)");

    // --- Configure input capture inversion before moving to ISR ---
    // NOTE: `receive_dshot_dma()` deferred until after `init_isr_state` —
    // GenericCapture's `dma_buf` is inside the struct, so its address changes
    // when `hal` is moved into IsrState. Arming DMA before the move sets
    // CMAR to a stack address that becomes stale after the move.
    {
        hal.input.set_inverted(BOARD.inverted_input);
    }

    // --- Build ISR state locally (all config applied before move) ---
    let mut isr_state = IsrState {
        commutation: Commutation::new(),
        bemf: BemfState::default(),
        duty: DutyState::default(),
        hal,
        cmd: rm32::dshot_commands::CommandProcessor::default(),
        edt: rm32::edt::EdtScheduler::default(),
        crsf: rm32::crsf::CrsfParser::new(),
        transfer: rm32::transfer::TransferState::default(),
        config: EepromConfig::default(),
        forward: true,
        edt_armed: false,
        edt_arm_enable: false,
        armed_timeout_count: 0,
        // Wide initial bounds — accept any plausible DSHOT frame until the
        // unarmed-idle averaging in transfer.rs:305 narrows the window.
        // Original 400/600 was too tight; rejected all real frames at any
        // reasonable PSC. See decode_frame at transfer.rs:255.
        frametime_low: 100,
        frametime_high: 60_000,
        voltage_based_ramp: BOARD.voltage_based_ramp,
    };

    // --- Build main loop state ---
    let mut main_state = MainState::new(
        &BOARD,
        rm32::main_state::ChipParams {
            timer1_max_arr: Chip::TIM1_AUTORELOAD,
            cpu_mhz: Chip::CPU_FREQUENCY_MHZ as u8,
        },
    );

    // --- Check bootloader device info for dynamic EEPROM address ---
    let eeprom_address = {
        const DEVINFO_MAGIC1: u32 = 0x5925_E3DA;
        const DEVINFO_MAGIC2: u32 = 0x4EB8_63D9;
        const DEVINFO_ADDR: u32 = 0x1000 - 32;
        // SAFETY: DEVINFO_ADDR points to a fixed bootloader info region in flash
        // (0x1000 - 32). This is memory-mapped, aligned, and always readable.
        let magic1 = unsafe { (DEVINFO_ADDR as *const u32).read_volatile() };
        let magic2 = unsafe { ((DEVINFO_ADDR + 4) as *const u32).read_volatile() };
        if magic1 == DEVINFO_MAGIC1 && magic2 == DEVINFO_MAGIC2 {
            const DEVICE_32K: u8 = 0x1F; // 32KB flash (F051)
            const DEVICE_64K: u8 = 0x35; // 64KB flash (G071)
            const DEVICE_128K: u8 = 0x2B; // 128KB flash (L431)
            // SAFETY: Magic validated above, so the bootloader info struct is present.
            // Offset 12 holds the device code byte; address is in flash, always readable.
            let device_code = unsafe { *((DEVINFO_ADDR + 8 + 4) as *const u8) };
            match device_code {
                DEVICE_32K => 0x0800_7C00u32,
                DEVICE_64K => 0x0800_F800u32,
                DEVICE_128K => 0x0801_F800u32,
                _ => Chip::EEPROM_START,
            }
        } else {
            Chip::EEPROM_START
        }
    };

    // --- Load EEPROM settings from flash ---
    let flash = FlashStorage::new();
    {
        flash.read(eeprom_address, main_state.config.as_bytes_mut());
    }
    // Validate and apply version migration
    if !main_state.config.is_valid() {
        main_state.config = EepromConfig::default();
    }
    main_state.config.apply_version_defaults();
    main_state.config.apply_comp_pwm_guard();
    main_state.config.apply_rc_car_overrides();

    // --- BENCH-DEBUG: clean baseline matching AM32 Configurator with all
    // "complex" features disabled. Mirrors the test setup used to compare
    // register-level parity with AM32 during smooth PWM motor operation.
    // Remove these overrides for production / EEPROM-respecting builds.
    main_state.config.stuck_rotor_protection = 0;
    main_state.config.stall_protection = 0;
    main_state.config.bi_direction = 0;
    main_state.config.use_sine_start = 0;
    main_state.config.brake_on_stop = 0;
    rm32_stm32::dprintln!("[rm32] BENCH: cleared stuck/stall/bidir/sine/brake");

    // Derive motor configuration from EEPROM + board (all math now in rm32, host-testable)
    let motor_cfg = main_state.config.derive_motor_config(
        Chip::TIM1_AUTORELOAD,
        BOARD.dead_time,
        BOARD.kv_divider,
        BOARD.startup_boost,
    );
    let minimum_duty_cycle = motor_cfg.minimum_duty;
    let min_startup_duty = motor_cfg.min_startup_duty;
    let startup_max_duty = motor_cfg.startup_max_duty;
    let timer1_max_arr = motor_cfg.timer1_max_arr;
    let dead_time_override = motor_cfg.dead_time_override;

    // Apply derived motor config to main state and PID controllers
    main_state.apply_motor_config(&motor_cfg);

    // Propagate loaded config to ISR state (still on stack, before move)
    isr_state.config = main_state.config;
    isr_state.forward = main_state.config.dir_reversed == 0;
    isr_state.edt_arm_enable = main_state.config.input_type() == rm32::config::InputType::EdtArm;
    isr_state
        .duty
        .set_duty_limits(minimum_duty_cycle, min_startup_duty, startup_max_duty);
    isr_state.duty.apply_max_ramp(main_state.config.max_ramp);
    if isr_state.config.eeprom_version > 0 {
        isr_state.transfer.servo.set_calibration(
            motor_cfg.servo_low,
            motor_cfg.servo_high,
            motor_cfg.servo_neutral,
            isr_state.config.servo_dead_band,
        );
    }
    if dead_time_override > 0 {
        isr_state.duty.apply_dead_time_override(dead_time_override);
        isr_state.hal.pwm.set_dead_time_override(dead_time_override);
    }

    // Move to static, then arm DMA (buffer address must be final).
    let isr = isr::init_isr_state(isr_state);
    #[cfg(not(feature = "benchuart"))]
    {
        isr.hal.input.receive_dshot_dma();
        rm32_stm32::dprintln!("[rm32] isr state installed, DMA armed");
    }
    // benchuart: PA2 belongs to USART2 RX — DShot capture is never armed.
    // Init AFTER input-capture GPIO setup so this owns PA2's final mux
    // (clock tree is at 80 MHz by now; BRR assumes it).
    #[cfg(feature = "benchuart")]
    {
        let _ = isr;
        rm32_stm32::bench_uart::init();
        rm32_stm32::dprintln!("[rm32] benchuart: USART2 RX @2M on PA2, DShot capture OFF");
    }

    // --- ADC + Telemetry (returned from init()) ---

    // Publish initial tim1_arr to SharedComm before ISR starts
    isr::shared().set_tim1_arr(timer1_max_arr);

    // --- Enable global interrupts ---
    // SAFETY: All ISR state has been initialized and moved to globals above.
    // NVIC priorities are configured. It is now safe to take interrupts.
    unsafe { cortex_m::interrupt::enable() };
    rm32_stm32::dprintln!("[rm32] irqs enabled, entering main loop");

    // --- Main loop ---
    let shared = isr::shared();
    let mut system = rm32::system::SystemTick::new();
    let mut log_counter: u32 = 0;
    // Bench safety guard: absolute vbat-sag + overcurrent kill, latched
    // until reset. See rm32_stm32::bench_guard for thresholds/rationale.
    #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
    let mut bench_guard = rm32_stm32::bench_guard::BenchGuard::new(Chip::CPU_FREQUENCY_MHZ);
    // Bench UART control state: parser + committed throttle + last-command
    // timestamp for the 3 s deadman (a dead host script must not leave
    // throttle latched — minz semantics).
    #[cfg(feature = "benchuart")]
    let mut bench_parser = rm32::bench_input::UartDuty::new();
    #[cfg(feature = "benchuart")]
    let mut bench_throttle: u16 = 0;
    #[cfg(feature = "benchuart")]
    let mut bench_last_cmd: Option<u32> = None;
    loop {
        // Bracket the per-iter main-loop body so we can measure how much of
        // the 50 µs TIM6 period is spent doing main work vs sleeping in wfi.
        // Excludes wfi (DWT keeps counting but main is asleep — that delta
        // is "until next IRQ", not work).
        #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
        let main_cyc_start = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
        log_counter = log_counter.wrapping_add(1);
        if log_counter.is_multiple_of(100_000) {
            // Decode detected input protocol from the three shared flags into
            // a single short tag for the log line. `dshot_telemetry` is
            // bidirectional DShot (RPM feedback), set independently of the
            // detection flags by the bidir-DShot handshake.
            let proto = match (shared.dshot(), shared.servo_pwm(), shared.dshot_telemetry()) {
                (true, _, true) => "BiDShot",
                (true, _, false) => "DShot",
                (false, true, _) => "PWM",
                (false, false, _) => "none",
            };
            // DWT cycle counter ÷ 1000 — readable monotonic timestamp that
            // confirms the main loop is still executing. Stalled if cyc_k
            // stops advancing between consecutive [loop n=] entries.
            // isr_tick — TIM6-ISR-driven counter (20 kHz). Compare its delta
            // to cyc_k delta to distinguish ISR-storm-starves-main from
            // total chip freeze. ~20 ISR ticks per ms of wall time expected.
            // DWT.CYCCNT only exists on Cortex-M3+ (not the M0/M0+ chips).
            #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
            let cyc_k = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() } / 1000;
            #[cfg(not(any(feature = "stm32l431", feature = "stm32g431")))]
            let cyc_k: u32 = 0;
            let isr_tick = shared.dbg_isr_tick();
            // Last-tick ISR duration (cycles). /80 ≈ µs at 80 MHz. A SAMPLE
            // from the most recent ISR, not a max — read at random points
            // across many ticks to see typical-vs-spike distribution.
            let t6_last = shared.dbg_tim6_last_cyc();
            let t14_last = shared.dbg_tim14_last_cyc();
            let comp_last = shared.dbg_comp_last_cyc();
            let dma_last = shared.dbg_dma_last_cyc();
            let exti_last = shared.dbg_exti_last_cyc();
            let main_last = shared.dbg_main_last_cyc();
            rm32_stm32::dprintln!(
                "[loop n={} cyc_k={} isr_tick={} t6={} t14={} comp={} dma={} exti={} main={}] proto={} mode={:?} newinput={} adj={} duty_set={} duty={} sig_to={} bemf_to_hap={} bemf_to={} zc={} ito={} stuck_prot={} hi_pin_n={} bidir_evt={} crc_pass={} crc_fail={} vbat_mv={} i_ma={}",
                log_counter / 100_000,
                cyc_k,
                isr_tick,
                t6_last,
                t14_last,
                comp_last,
                dma_last,
                exti_last,
                main_last,
                proto,
                shared.motor_mode(),
                shared.newinput(),
                shared.adjusted_input(),
                shared.duty_cycle_setpoint(),
                shared.duty_cycle(),
                shared.signal_timeout(),
                main_state.protection.bemf_timeout_happened(),
                main_state.protection.bemf_timeout(),
                shared.zero_crosses(),
                shared.interval_timer_count(),
                main_state.config.stuck_rotor_protection,
                shared.dbg_high_pin_n(),
                shared.dbg_bidir_evt(),
                shared.dbg_crc_pass(),
                shared.dbg_crc_fail(),
                shared.battery_voltage(),
                shared.actual_current(),
            );
            // Dump recent frame snapshots (mix of pass + fail). Useful for
            // catching DMA buffer alignment / edge polarity issues in bidir.
            #[cfg(feature = "debuguart")]
            for snap in rm32_stm32::dbg_frame_history::take().iter() {
                let d1 = (snap.buf[1] as u16).wrapping_sub(snap.buf[0] as u16);
                let d2 = (snap.buf[2] as u16).wrapping_sub(snap.buf[1] as u16);
                let d3 = (snap.buf[3] as u16).wrapping_sub(snap.buf[2] as u16);
                let d4 = (snap.buf[4] as u16).wrapping_sub(snap.buf[3] as u16);
                let d5 = (snap.buf[5] as u16).wrapping_sub(snap.buf[4] as u16);
                let d6 = (snap.buf[6] as u16).wrapping_sub(snap.buf[5] as u16);
                let d7 = (snap.buf[7] as u16).wrapping_sub(snap.buf[6] as u16);
                rm32_stm32::dprintln!(
                    "[snap n={} pass={} bidir={}] buf={} {} {} {} {} {} {} {} | d= {} {} {} {} {} {} {}",
                    snap.n,
                    snap.crc_pass as u8,
                    snap.bidir as u8,
                    snap.buf[0],
                    snap.buf[1],
                    snap.buf[2],
                    snap.buf[3],
                    snap.buf[4],
                    snap.buf[5],
                    snap.buf[6],
                    snap.buf[7],
                    d1,
                    d2,
                    d3,
                    d4,
                    d5,
                    d6,
                    d7,
                );
            }
        }
        // Sine mode stepping (shared with harness via SystemTick).
        // TIM1 CCR writes go directly to MMIO — no ISR state access needed.
        if let Some((result, (ch1, ch2, ch3))) = system.tick_sine(
            shared,
            &main_state.config,
            BOARD.dead_time as i16,
            Chip::TIM1_AUTORELOAD,
        ) {
            // Sine PWM: direct TIM1 CCR writes — safe from main context
            // (atomic register writes, no ISR state access needed).
            rm32_stm32::mcu::write_tim1_ccr(ch1, ch2, ch3);
            match result {
                rm32::sine::SineStepResult::Continue(delay_us) => {
                    sys.delay_micros(delay_us as u32);
                }
                rm32::sine::SineStepResult::Changeover {
                    commutation_interval,
                    step,
                } => {
                    system.apply_sine_changeover(shared, &mut main_state, commutation_interval);
                    // Publish changeover step — ISR applies com_step + enables interrupts
                    shared.set_changeover_step(step);
                }
                rm32::sine::SineStepResult::Idle => {}
            }
        }

        // Shared pipeline — ISR runs async, sync via SharedState atomics.
        system.run_tick(shared, &mut main_state, &mut adc, &mut telem, || {});

        // Bench safety guard: evaluate + enforce. On trip: AllOff + Disarm,
        // then RE-ASSERTED every pass while latched (IsrAction is cleared by
        // the ISR after acting; DShot input could otherwise re-arm). Only a
        // reset re-arms the guard — a kill is evidence, not a hiccup.
        #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
        {
            let guard_now = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
            if let Some(reason) = bench_guard.tick(
                guard_now,
                shared.running(),
                shared.battery_voltage(),
                shared.actual_current(),
            ) {
                let tag = match reason {
                    rm32_stm32::bench_guard::KillReason::Overcurrent => "OC",
                    rm32_stm32::bench_guard::KillReason::VbatSag => "VBAT",
                };
                rm32_stm32::dprintln!(
                    "!! BENCH KILL reason={} vbat_mv={} i_ma={} (latched until reset)",
                    tag,
                    shared.battery_voltage(),
                    shared.actual_current()
                );
            }
            if bench_guard.latched().is_some() {
                shared.request_isr_action(rm32::shared_comm::IsrAction::AllOff);
                shared.transition(rm32::motor_mode::MotorEvent::Disarm);
            }
        }

        // Bench UART control band: drain the RX ring, parse, inject throttle
        // through the same shared-state path DShot uses (harness.rs model:
        // set_newinput + signal_timeout=0 each pass while fresh, so arming /
        // ramp / LVC / timeout semantics are identical). Deadman: 3 s without
        // a command zeroes throttle and lets the firmware signal timeout run.
        #[cfg(feature = "benchuart")]
        {
            use rm32::bench_input::UartCmd;
            let bench_now = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
            let rx = rm32_stm32::bench_uart::ring();
            while let Some(b) = rx.pop() {
                if let Some(cmd) = bench_parser.step(b) {
                    match cmd {
                        UartCmd::SetThrottle(v) => {
                            bench_throttle = v;
                            bench_last_cmd = Some(bench_now);
                        }
                        UartCmd::Stop => {
                            bench_throttle = 0;
                            bench_last_cmd = Some(bench_now);
                        }
                        UartCmd::Kill => {
                            bench_throttle = 0;
                            bench_last_cmd = Some(bench_now);
                            shared.request_isr_action(rm32::shared_comm::IsrAction::AllOff);
                            rm32_stm32::dprintln!("[bench] KILL (w): all off, throttle 0");
                        }
                        UartCmd::Info => {
                            rm32_stm32::dprintln!(
                                "[i] mode={:?} in={} adj={} duty={} zc={} ci={} vbat_mv={} i_ma={} guard={}",
                                shared.motor_mode(),
                                bench_throttle,
                                shared.adjusted_input(),
                                shared.duty_cycle(),
                                shared.zero_crosses(),
                                shared.commutation_interval(),
                                shared.battery_voltage(),
                                shared.actual_current(),
                                bench_guard.latched().is_some() as u8
                            );
                        }
                        UartCmd::TraceToggle => {
                            rm32_stm32::dprintln!("[bench] zctrace: not ported yet (rung 4)");
                        }
                        UartCmd::BbDump => {
                            rm32_stm32::dprintln!("[bench] blackbox: not ported yet (rung 3)");
                        }
                    }
                }
            }
            // A latched safety kill outranks any commanded throttle.
            if bench_guard.latched().is_some() {
                bench_throttle = 0;
            }
            let deadman_cyc: u32 = 3 * Chip::CPU_FREQUENCY_MHZ * 1_000_000;
            if let Some(last) = bench_last_cmd {
                if bench_now.wrapping_sub(last) < deadman_cyc {
                    shared.set_input_set(true);
                    shared.set_newinput(bench_throttle);
                    shared.set_signal_timeout(0);
                } else {
                    bench_last_cmd = None;
                    bench_throttle = 0;
                    shared.set_newinput(0);
                    rm32_stm32::dprintln!("[bench] deadman (3s): throttle zeroed");
                }
            }
        }

        // Arming feedback: LED only (beeps need HAL access — TODO: tone request via SharedState)
        if main_state.just_armed && BOARD.has_led {
            led.set_status(LedStatus::Armed);
        }

        // WS2812 LED error indicator
        if BOARD.has_led {
            // Error LED on BEMF timeout (stuck rotor)
            if main_state.protection.bemf_timeout_happened() > main_state.protection.bemf_timeout()
                && main_state.config.stuck_rotor_protection != 0
            {
                led.set_status(LedStatus::Error);
            }
        }

        // Dynamic IRQ priority: swap DShot DMA vs commutation priority based on RPM.
        // Low eRPM: DShot DMA > commutation (don't drop input frames)
        // High eRPM: commutation > DShot (don't miss commutation steps)
        // No-op on most MCUs; L431 (M4F with preemption) does the actual swap.
        rm32_stm32::mcu::adjust_irq_priorities(
            shared.commutation_interval(),
            shared.dshot_telemetry(),
        );

        // EEPROM save on DShot command
        // TODO: ISR command processor mutates its own config copy. Need to
        // publish changed fields via SharedState for main to persist correctly.
        // For now, saves main_state.config (synced from EEPROM at boot).
        if shared.save_settings_flag() {
            shared.set_save_settings_flag(false);
            let mut flash = FlashStorage::new();
            flash.write(eeprom_address, main_state.config.as_bytes());
        }

        // ESC info response on DShot command
        if shared.send_esc_info_flag() {
            shared.set_send_esc_info_flag(false);
            let mut info_pkt = [0u8; 49];
            rm32::telemetry::make_info_packet(&mut info_pkt, main_state.config.as_bytes());
            telem.send_dma(&info_pkt);
        }

        // Self-reset when signal_timeout fires (AM32 main.c:1892-1918 behavior).
        // Drains debug UART first so any in-flight banner / log byte lands
        // before the chip restarts. SCB::sys_reset sets SFTRSTF → bootloader
        // skips first-chance signal-pin check → DFU loop activates → BF
        // passthrough / AM32 Configurator BLHeli protocol can connect.
        // BENCH DEBUG: self-reset on signal_timeout is neutered here so the
        // chip stays alive during bidir-DSHOT detection investigation.
        // Print rate-limited (every 200k iters) so the UART doesn't choke the
        // main loop. Restore `sys.reset();` to re-enable Configurator passthrough.
        if main_state.needs_reset {
            main_state.needs_reset = false;
            if log_counter.is_multiple_of(200_000) {
                rm32_stm32::dprintln!("[rm32] signal_timeout sticky (RESET SUPPRESSED)");
            }
            // sys.reset();
        }

        sys.reload_watchdog();
        #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
        {
            let main_cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
            shared.dbg_main_last_cyc_set(main_cyc_end.wrapping_sub(main_cyc_start));
        }
        // Spin instead of wfi — matches AM32's free-running main loop
        // (main.c:1843, no sleep). wfi was an invented Rust-embedded
        // idiom that gated main rate to the ISR rate, which (a) created
        // a divergence from AM32's architecture and (b) sometimes
        // interferes with SWD attach + RTT. 1 kHz dispatch correctness
        // no longer depends on main rate — the counter is incremented
        // in ten_khz_tick (TIM6 ISR) at 20 kHz.
        cortex_m::asm::nop();
    }
}
