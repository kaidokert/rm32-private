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
use rm32::hal::{PwmOutput, System, TelemetryUart as _};
use rm32::ws2812::LedStatus;

use rm32::main_state::MainState;
use rm32_stm32::init::InitResult;
use rm32_stm32::isr::{self, IsrState};
use rm32_stm32::mcu::FlashStorage;
use rm32_stm32::mcu::{Chip, ChipConfig};

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
        use rm32::sounds::Sounds;
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
    // Bench-debug: IWDG disabled so the chip can sit idle without resetting
    // itself between test runs. Re-enable for production.
    // sys.start_watchdog(Chip::WDG_PRESCALER, Chip::WDG_RELOAD);
    rm32_stm32::dprintln!("[rm32] wdg DISABLED (bench debug)");

    // --- Configure input capture inversion before moving to ISR ---
    // NOTE: `receive_dshot_dma()` deferred until after `init_isr_state` —
    // GenericCapture's `dma_buf` is inside the struct, so its address changes
    // when `hal` is moved into IsrState. Arming DMA before the move sets
    // CMAR to a stack address that becomes stale after the move.
    {
        use rm32::hal::InputCapture;
        hal.input.set_inverted(BOARD.inverted_input);
    }

    // --- Build ISR state and move to global ---
    let isr_state = IsrState {
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
        edt_arm_enable: false, // set from EEPROM after config load
        armed_timeout_count: 0,
        frametime_low: 400,
        frametime_high: 600,
        voltage_based_ramp: BOARD.voltage_based_ramp,
    };
    isr::init_isr_state(isr_state);
    rm32_stm32::dprintln!("[rm32] isr state installed");

    // Now arm DMA capture with the buffer at its final static address.
    isr::with_isr_state_boot(|isr| {
        use rm32::hal::InputCapture;
        isr.hal.input.receive_dshot_dma();
    });
    rm32_stm32::dprintln!("[rm32] input dma armed (post-move)");

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
        use rm32::hal::Flash as _;
        flash.read(eeprom_address, main_state.config.as_bytes_mut());
    }
    // Validate and apply version migration
    if !main_state.config.is_valid() {
        main_state.config = EepromConfig::default();
    }
    main_state.config.apply_version_defaults();
    main_state.config.apply_comp_pwm_guard();
    main_state.config.apply_rc_car_overrides();

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

    // Propagate loaded config to ISR state (before interrupts enabled)
    isr::with_isr_state_boot(|isr| {
        isr.config = main_state.config;
        isr.forward = main_state.config.dir_reversed == 0;
        isr.edt_arm_enable = main_state.config.input_type() == rm32::config::InputType::EdtArm;
        // Apply timer1_max_arr from pwm_frequency config (ISR reads from SharedComm)
        // Apply startup duty from EEPROM
        isr.duty
            .set_duty_limits(minimum_duty_cycle, min_startup_duty, startup_max_duty);
        isr.duty.apply_max_ramp(main_state.config.max_ramp);
        // Apply servo EEPROM calibration to transfer state
        if isr.config.eeprom_version > 0 {
            isr.transfer.servo.set_calibration(
                motor_cfg.servo_low,
                motor_cfg.servo_high,
                motor_cfg.servo_neutral,
                isr.config.servo_dead_band,
            );
        }
        // Apply dead-time override to duty thresholds
        if dead_time_override > 0 {
            isr.duty.apply_dead_time_override(dead_time_override);
        }
    });

    // Apply dead-time override via PwmOutput trait
    if dead_time_override > 0 {
        isr::with_isr_state_boot(|isr| {
            isr.hal.pwm.set_dead_time_override(dead_time_override);
        });
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
    loop {
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
            rm32_stm32::dprintln!(
                "[loop] proto={} mode={:?} newinput={} adj={} duty_set={} duty={} sig_to={} bemf_to_hap={} bemf_to={} zc={} ito={} stuck_prot={}",
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
            );
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
            use rm32::hal::Flash as _;
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
        if main_state.needs_reset {
            rm32_stm32::dprintln!("[rm32] signal_timeout → sys_reset");
            #[cfg(feature = "debuguart")]
            rm32_stm32::debug_uart::flush();
            sys.reset();
        }

        sys.reload_watchdog();
        cortex_m::asm::wfi();
    }
}
