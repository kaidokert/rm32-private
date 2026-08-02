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

    // Disarm any leftover DWT watchpoint + monitor enable FIRST. The
    // debug domain (DWT comparators, DEMCR.MON_EN) survives SYSTEM
    // resets — only power-on clears it — so a watchpoint armed by a
    // previous firmware/debug session otherwise halts EVERY boot at the
    // first write to the watched address ("dead chip" that flashing
    // cannot fix). 'V' re-arms deliberately.
    unsafe {
        let dwt = &*cortex_m::peripheral::DWT::PTR;
        // Disarm ALL DWT comparators, not just c[1]. M4 (L431/G431) has
        // up to 4 (NUMCOMP = CTRL[31:28]); a leftover watchpoint on ANY
        // of them survives a system reset (power-on only clears the
        // debug domain) and halts the core on its watched write — the
        // "dead chip flashing can't fix" scar. The old code only cleared
        // c[1] (the slot 'V' arms); a watchpoint latched on c[0]/c[2]/
        // c[3] by a gdb/probe session sailed through. Latent-bug
        // hygiene, closed 07-28. 'V' re-arms c[1] deliberately later.
        let numcomp = ((dwt.ctrl.read() >> 28) & 0xF) as usize;
        for i in 0..numcomp.min(4) {
            dwt.c[i].function.write(0);
            dwt.c[i].comp.write(0);
            dwt.c[i].mask.write(0);
        }
        let dcb = &*cortex_m::peripheral::DCB::PTR;
        dcb.demcr.modify(|v| v & !(1 << 16)); // MON_EN off
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
    // Re-print the reset cause now that the UART runs at its final baud —
    // the pre-clock-config print above lands as garbage on the wire
    // (RTT-only). Rung 2: last-boot reason visible on the bench wire.
    for label in reset_cause.iter_labels() {
        rm32_stm32::dprintln!("[rm32] last reset: {}", label);
    }

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
        tone: rm32::tone::ToneScheduler::default(),
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

    // --- BENCH: deterministic factory-baseline config, PERSISTED ---
    // History: the flash page held `01 03` + an erased-0xFF body — "valid"
    // to is_valid() but garbage values (kv 10220, poles 255, startup 255…),
    // each pinning a different ceiling (eRPM envelope floor 600, startup
    // caps, temp map). Instead of chasing overrides field by field, build
    // the AM32-configurator-style bench baseline in code and WRITE it to
    // the flash page, so the stored config and the running config are the
    // same deterministic thing (and future EEPROM-respecting builds read
    // sane values). Complex features stay off (zeroed): stuck/stall/bidir/
    // sine/brake. Bench-only: a production build must NEVER overwrite the
    // user's stored configuration.
    #[cfg(feature = "benchuart")]
    {
        let mut desired = EepromConfig::default();
        desired.apply_version_defaults();
        desired.reserved_0 = 1; // byte 0 = the bootloader's jump-enable flag
        desired.version_major = 2;
        desired.version_minor = 20;
        desired.comp_pwm = 1; // damped (complementary) PWM — AM32 default
        desired.variable_pwm = 1; // variable carrier — AM32 default (A/B exonerated it in the surge hunt)
        desired.advance_level = 26; // temp_advance() -> 16 = 15 deg
        desired.temperature_limit = 141; // disabled (AM32 configurator default)
        desired.motor_kv = 55; // 55*40+20 = 2220 kv
        desired.motor_poles = 14;
        desired.minimum_duty_cycle = 4; // -> minimum_duty 40
        desired.startup_power = 105; // -> min_startup 145, startup_max 440
        if main_state.config.as_bytes() != desired.as_bytes() {
            let mut flashw = FlashStorage::new();
            flashw.write(eeprom_address, desired.as_bytes());
            rm32_stm32::dprintln!("[rm32] BENCH: persisted factory-baseline config");
        } else {
            rm32_stm32::dprintln!("[rm32] BENCH: stored config already at baseline");
        }
        main_state.config = desired;
    }
    #[cfg(feature = "benchuart")]
    {
        let cfg_bytes = main_state.config.as_bytes();
        for (i, chunk) in cfg_bytes.chunks(16).enumerate() {
            let mut line = [0u8; 48];
            let mut n = 0;
            for b in chunk {
                let hi = b >> 4;
                let lo = b & 0xf;
                line[n] = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 };
                line[n + 1] = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 };
                line[n + 2] = b' ';
                n += 3;
            }
            rm32_stm32::dprintln!(
                "[cfg {:02}] {}",
                i * 16,
                core::str::from_utf8(&line[..n]).unwrap_or("?")
            );
        }
    }

    // Derive motor configuration from EEPROM + board (all math now in rm32, host-testable)
    #[allow(unused_mut)]
    let mut motor_cfg = main_state.config.derive_motor_config(
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
    // comp_pwm reaches the phase driver HERE and nowhere else — the
    // per-MCU init constructed it with false (safe idle). See
    // PhaseDriver::set_comp_pwm for the failure mode when this is missed.
    isr_state
        .hal
        .phase
        .set_comp_pwm(main_state.config.comp_pwm != 0);
    rm32_stm32::dprintln!("[rm32] comp_pwm wired: {}", main_state.config.comp_pwm != 0);
    // BENCH: boot in diode drive regardless of config — the complementary
    // spin-up currently churns (comp-on climb is the open divergence);
    // the 'D' command flips drive live once at a clean operating point.
    #[cfg(feature = "benchuart")]
    {
        // AUTO: complementary in interrupt mode, diode during polling/
        // recovery. Sustained-comp parity 3/4 x 15s at clone level
        // (7.3-7.7 e/w) where always-comp fell into a permanent ~245Hz
        // churn attractor: comp drive's braking during polling-grind
        // recovery made the churn self-sustaining. Designed divergence
        // from AM32 (which applies comp unconditionally) — measured and
        // chosen; 'D' cycles diode/comp/auto live.
        rm32_stm32::phase::COMP_PWM_LIVE.store(3, core::sync::atomic::Ordering::Relaxed);
        rm32_stm32::dprintln!("[rm32] bench drive: AUTO at boot ('D' cycles)");
    }

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

    // Hardware-timed injected current sampling (clone FALCON pattern):
    // per-PWM-cycle current in JDR1, read per commutation into the
    // probe row. High-rate input-current visibility for desync-vs-sag
    // cause/effect (operator directive).
    // DEFAULT OFF: armed 24 kHz injected conversions preempt the regular
    // vbat/current control scan and measurably degrade climbing (0/8
    // climbs with it armed at boot vs 2/4 without, 07-25). 'J' arms it
    // live for capture/autopsy sessions — instruments cost only when
    // attached.
    #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
    rm32_stm32::dprintln!("[rm32] injected current sampling: OFF ('J' arms)");

    // --- ADC + Telemetry (returned from init()) ---

    // Publish initial tim1_arr to SharedComm before ISR starts
    isr::shared().set_tim1_arr(timer1_max_arr);

    // debuguart: PA0 soft-UART RX (host->ESC bench input; USB-TTL TX on
    // header pin 4). Decoded bytes drain into the main loop below.
    #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
    let mut softuart_rx = rm32_stm32::mcu_l431::softuart_rx::init();
    // Last byte + count, surfaced via the [su] heartbeat line: the host
    // shares one adapter between 115200 TX-log and 9600 RX, so an
    // immediate echo is transmitted while the host is still parked at
    // 9600 — a latched heartbeat readback has no such race.
    #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
    let (mut su_last, mut su_n): (u8, u32) = (0, 0);
    // Soft-UART command dispatch state: the shared bench_input parser +
    // the latched config offset for 'o'/'v' write pairs. Execution is
    // DEFERRED ~150 ms (except Kill): the host shares one adapter
    // between 9600 TX and 115200 RX-log, and an immediate reply
    // transmits before it can switch baud back to listen.
    #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
    let mut su_parser = rm32::bench_input::UartDuty::new();
    #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
    let mut su_cfg_offset: u8 = 0;
    #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
    let mut su_pending: Option<(rm32::bench_input::UartCmd, u32)> = None;
    #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
    rm32_stm32::dprintln!("[rm32] softuart RX: PA0 9600 8N1 (EXTI0+LPTIM1)");

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
    // DEBUG BUILDS ONLY (Tier A5): the thresholds are bench-pack facts
    // (14.0 V OV, 15 A OC) — a 4S production pack would trip the latched
    // OVOLT kill within ~60 ms of classification. Production protection
    // is the AM32 mechanism set (LVC, current-limit PID, stuck rotor).
    #[cfg(all(
        feature = "debuguart",
        any(feature = "stm32l431", feature = "stm32g431")
    ))]
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
    #[cfg(feature = "benchuart")]
    let mut bench_cfg_offset: u8 = 0;
    // Decision counters for the phantom-stop hunt: every committed value
    // and every Stop commit is counted — a host streaming "50\n" should
    // produce vals only; stops>0 means RX corruption turned a throttle
    // line into a commanded stop (the silent-veto class).
    #[cfg(feature = "benchuart")]
    let mut bench_stop_n: u32 = 0;
    #[cfg(feature = "benchuart")]
    let mut bench_val_n: u32 = 0;
    #[cfg(feature = "benchuart")]
    let mut bench_last_val: u16 = 0;
    #[cfg(feature = "benchuart")]
    let mut bench_drops: u32 = 0;
    // Two-frame confirmation (AM32 protocol-detection pattern): a
    // throttle/stop commit only APPLIES when the same value arrives twice
    // consecutively. Measured need: ore=24 stops=13 in one sweep — RX
    // overrun under prio-0 ISR bursts drops a digit and "50\n" becomes
    // "0\n", a commanded stop at speed (brake -> regen pump -> OVOLT).
    // Hosts stream the setpoint at ~10 Hz, so a real change applies one
    // repeat (~100 ms) later; singleton corruptions never apply.
    // 's'/'w' stay immediate.
    #[cfg(feature = "benchuart")]
    let mut bench_pending: u16 = 0;
    #[cfg(feature = "benchuart")]
    let mut bench_veto_n: u32 = 0;
    // Blackbox mode tracker: record a MOD event whenever the packed
    // armed/running/old_routine/stepper_sine bits change.
    #[cfg(feature = "blackbox")]
    let mut bb_last_mode: u16 = 0xFFFF;
    loop {
        // Bracket the per-iter main-loop body so we can measure how much of
        // the 50 µs TIM6 period is spent doing main work vs sleeping in wfi.
        // Excludes wfi (DWT keeps counting but main is asleep — that delta
        // is "until next IRQ", not work).
        #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
        let main_cyc_start = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
        log_counter = log_counter.wrapping_add(1);
        // Heartbeat suppressed while the motor runs (comp-degradation
        // suspect test: the first [loop] print lands ~5 s after boot —
        // exactly when sustained-comp runs collapse; RTT's fragmented
        // critical sections + the 300-byte line are the suspects).
        if log_counter.is_multiple_of(100_000) && !shared.running() {
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
                "[loop n={} cyc_k={} isr_tick={} t6={} t14={} comp={} dma={} exti={} main={}] proto={} mode={:?} newinput={} adj={} duty_set={} duty={} sig_to={} bemf_to_hap={} bemf_to={} zc={} ito={} stuck_prot={} hi_pin_n={} bidir_evt={} crc_pass={} crc_fail={} vbat_mv={} i_ma={} dmax={} ecom={} degC={}",
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
                shared.duty_maximum(),
                shared.e_com_time(),
                shared.degrees_celsius(),
            );
            // Flight-recorder tail: newest sample (ci, mA, mV). Doubles as
            // the rings' live consumer — without a reachable read, LTO
            // dead-store-eliminates the write-only SR arrays entirely
            // (observed: debuguart image lost SR_CI/MA/MV, kept SR_HEAD).
            #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
            {
                use core::sync::atomic::Ordering;
                use rm32_stm32::isr_handlers as ih;
                let h = ih::SR_HEAD.load(Ordering::Relaxed) as usize;
                let i = h.checked_sub(1).map(|v| v % ih::SR_N).unwrap_or(0);
                rm32_stm32::dprintln!(
                    "[sr n={} ci={} ma={} mv={} edt={} edtv={:#05x} dsy={} exc={} wex={} cm={}]",
                    h,
                    ih::SR_CI[i].load(Ordering::Relaxed),
                    ih::SR_MA[i].load(Ordering::Relaxed),
                    ih::SR_MV[i].load(Ordering::Relaxed),
                    ih::EDT_SENT.load(Ordering::Relaxed),
                    ih::EDT_LAST.load(Ordering::Relaxed),
                    main_state.desync_events,
                    ih::EXC_N.load(Ordering::Relaxed),
                    ih::EXC_MAX.load(Ordering::Relaxed),
                    ih::EXC_COMMS.load(Ordering::Relaxed)
                );
            }
            // PA0 soft-UART health: decoded frames / framing errors /
            // queue overruns, plus main's drain count + last byte.
            #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
            {
                let (f, e, o) = rm32_stm32::mcu_l431::softuart_rx::counters();
                use core::sync::atomic::Ordering;
                use rm32_stm32::isr_handlers as ihh;
                rm32_stm32::dprintln!(
                    "[su f={} e={} o={} n={} last={:#04x} tn={}/{}/{}]",
                    f,
                    e,
                    o,
                    su_n,
                    su_last,
                    ihh::TONE_STARTS.load(Ordering::Relaxed),
                    ihh::TONE_ABORTS.load(Ordering::Relaxed),
                    ihh::TONE_ENDS.load(Ordering::Relaxed)
                );
            }
            // Edge-probe lifetime totals — veto visibility even with the
            // zct stream off (instrument-decisions-not-outcomes).
            #[cfg(feature = "zctrace")]
            {
                let (gc, pr) = rm32_stm32::edge_probe::totals();
                rm32_stm32::dprintln!("[veto gated={} prej={}]", gc, pr);
            }
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

        // Blackbox: mode-transition events (Running <-> OldRoutine
        // oscillation is exactly what the chop investigation needs to see).
        #[cfg(feature = "blackbox")]
        {
            let mode_bits = (shared.armed() as u16)
                | ((shared.running() as u16) << 1)
                | ((shared.old_routine() as u16) << 2)
                | ((shared.stepper_sine() as u16) << 3);
            if mode_bits != bb_last_mode {
                bb_last_mode = mode_bits;
                rm32_stm32::bench_bb::record(rm32::blackbox::EV_MOD, 0, mode_bits);
            }
        }

        // Bench safety guard: evaluate + enforce. On trip: AllOff + Disarm,
        // then RE-ASSERTED every pass while latched (IsrAction is cleared by
        // the ISR after acting; DShot input could otherwise re-arm). Only a
        // reset re-arms the guard — a kill is evidence, not a hiccup.
        // Debug builds only — see the instantiation comment (Tier A5).
        #[cfg(all(
            feature = "debuguart",
            any(feature = "stm32l431", feature = "stm32g431")
        ))]
        {
            let guard_now = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
            if let Some(reason) = bench_guard.tick(
                guard_now,
                shared.running(),
                shared.battery_voltage(),
                shared.actual_current(),
            ) {
                let (tag, code) = match reason {
                    rm32_stm32::bench_guard::KillReason::Overcurrent => ("OC", 1u16),
                    rm32_stm32::bench_guard::KillReason::VbatSag => ("VBAT", 2u16),
                    rm32_stm32::bench_guard::KillReason::OverVolt => ("OVOLT", 3u16),
                };
                // Blackbox: record the kill, then FREEZE so the dump shows
                // the events leading TO the fault (minz reason codes:
                // 1=OC, 2=vbat).
                #[cfg(feature = "blackbox")]
                {
                    rm32_stm32::bench_bb::record(rm32::blackbox::EV_KIL, 0, code);
                    rm32_stm32::bench_bb::freeze();
                }
                #[cfg(not(feature = "blackbox"))]
                let _ = code;
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
            // WAXWING freeze-on-fall: any desync/orbit event at wall
            // duty freezes the phase-voltage ring so the deaf window
            // survives the churn (52 ms post-mortem; 'x' dumps + re-arms).
            #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
            {
                static mut WAX_LAST_EVT: u32 = 0;
                let evt = main_state
                    .desync_events
                    .wrapping_add(main_state.orbit_trips);
                let last = unsafe { WAX_LAST_EVT };
                if evt != last {
                    unsafe { WAX_LAST_EVT = evt };
                    // No ci gate: the 98-100% battery-wall storm is the
                    // FAST-rotor desync class (onset at ci~109, stays in
                    // interrupt mode so the OldRoutine trigger below never
                    // fires either). duty>1550 already scopes to the wall;
                    // the old `ci > 300` gate excluded exactly the storm
                    // this instrument exists to capture.
                    if shared.duty_cycle() > 1550
                        && !rm32_stm32::mcu_l431::adc::wax_frozen()
                        && rm32_stm32::mcu_l431::adc::wax_freeze()
                    {
                        rm32_stm32::dprintln!("[wax] FROZEN on fall (dsy+otrip={})", evt);
                    }
                }
                // Freeze on the Running->OldRoutine lock-loss transition.
                // At the wall the chop is a BEMF lock loss to OldRoutine,
                // NOT a desync (drop stays 0), so the trigger above never
                // fires. THIS transition IS the deaf window: the arc-vs-
                // VALUE bedrock test wants the ring frozen right here.
                static mut WAX_LAST_OLD: bool = false;
                let now_old = shared.old_routine();
                let was_old = unsafe { WAX_LAST_OLD };
                unsafe { WAX_LAST_OLD = now_old };
                if now_old && !was_old {
                    // Count EVERY Running->OldRoutine drop (operator ask:
                    // safe-mode drops must be logged, not sample-lucky).
                    // Edge-detected at main-loop rate; printed as drops=.
                    bench_drops = bench_drops.wrapping_add(1);
                    if bench_last_val > 1500
                        && !rm32_stm32::mcu_l431::adc::wax_frozen()
                        && rm32_stm32::mcu_l431::adc::wax_freeze()
                    {
                        rm32_stm32::dprintln!(
                            "[wax] FROZEN on lock-loss (Running->Old, in={})",
                            bench_last_val
                        );
                    }
                }
            }
            rm32_stm32::bench_uart::drain_dma();
            let rx = rm32_stm32::bench_uart::ring();
            while let Some(b) = rx.pop() {
                if let Some(cmd) = bench_parser.step(b) {
                    match cmd {
                        UartCmd::SetThrottle(v) => {
                            bench_last_cmd = Some(bench_now);
                            bench_val_n += 1;
                            bench_last_val = v;
                            if v == bench_pending {
                                bench_throttle = v;
                            } else {
                                bench_veto_n += 1;
                            }
                            bench_pending = v;
                        }
                        UartCmd::Stop => {
                            bench_last_cmd = Some(bench_now);
                            bench_stop_n += 1;
                            if bench_pending == 0 {
                                bench_throttle = 0;
                            } else {
                                bench_veto_n += 1;
                            }
                            bench_pending = 0;
                        }
                        UartCmd::Kill => {
                            bench_throttle = 0;
                            bench_last_cmd = Some(bench_now);
                            shared.request_isr_action(rm32::shared_comm::IsrAction::AllOff);
                            rm32_stm32::dprintln!("[bench] KILL (w): all off, throttle 0");
                        }
                        UartCmd::Info => {
                            // minz-EXACT wire format — map_sweep.py / fly.py /
                            // cmp_report.py regex this line. avg is in 0.5 µs
                            // interval ticks (f_e = 2e6/(6*avg)); iraw/vbat are
                            // raw ADC counts (scripts apply the sense cal:
                            // 26.86 mA/count, 7.52 mV/count — inverted here
                            // from our mA/mV). step isn't published to shared
                            // (scripts don't capture it) — 0. drop/guard are
                            // zctrace fields (rung 4) — 0. killed = bench_guard
                            // latch.
                            let avg = (shared.e_com_time() / 3).max(0) as u32;
                            let iraw = (shared.actual_current().max(0) as u32) * 100 / 2686;
                            let vraw = (shared.battery_voltage() as u32) * 100 / 752;
                            rm32_stm32::dprintln!(
                                "i step=0 old={} run={} ci={} avg={} zc={} duty={} iraw={} vbat={} drop=0 guard=0 killed={} ore={} stops={} vals={} lastv={} veto={} dsy={} otrip={} arr={} f={} dc={} ds={} do={} drops={} exc={} wex={} cm={}",
                                shared.old_routine() as u8,
                                shared.running() as u8,
                                shared.commutation_interval(),
                                avg,
                                shared.zero_crosses(),
                                shared.duty_cycle(),
                                iraw,
                                vraw,
                                bench_guard.latched().is_some() as u8,
                                rm32_stm32::bench_uart::ore_count(),
                                bench_stop_n,
                                bench_val_n,
                                bench_last_val,
                                bench_veto_n,
                                main_state.desync_events,
                                main_state.orbit_trips,
                                shared.tim1_arr(),
                                main_state.dsy_fast,
                                main_state.dsy_demote_cur,
                                main_state.dsy_demote_slow,
                                main_state.dsy_demote_old,
                                bench_drops,
                                {
                                    #[cfg(feature = "stm32l431")]
                                    {
                                        rm32_stm32::isr_handlers::EXC_N
                                            .load(core::sync::atomic::Ordering::Relaxed)
                                    }
                                    #[cfg(not(feature = "stm32l431"))]
                                    {
                                        0u32
                                    }
                                },
                                {
                                    #[cfg(feature = "stm32l431")]
                                    {
                                        rm32_stm32::isr_handlers::EXC_MAX
                                            .load(core::sync::atomic::Ordering::Relaxed)
                                    }
                                    #[cfg(not(feature = "stm32l431"))]
                                    {
                                        0u32
                                    }
                                },
                                {
                                    #[cfg(feature = "stm32l431")]
                                    {
                                        rm32_stm32::isr_handlers::EXC_COMMS
                                            .load(core::sync::atomic::Ordering::Relaxed)
                                    }
                                    #[cfg(not(feature = "stm32l431"))]
                                    {
                                        0u32
                                    }
                                }
                            );
                        }
                        UartCmd::TraceToggle => {
                            #[cfg(feature = "zctrace")]
                            {
                                let on = rm32_stm32::bench_zct::toggle();
                                rm32_stm32::dprintln!(
                                    "[bench] zctrace {} (drops={})",
                                    if on { "ON" } else { "OFF" },
                                    rm32_stm32::bench_zct::drop_count()
                                );
                            }
                            #[cfg(not(feature = "zctrace"))]
                            rm32_stm32::dprintln!(
                                "[bench] zctrace: build without 'zctrace' feature"
                            );
                        }
                        UartCmd::DriveToggle => {
                            use core::sync::atomic::Ordering;
                            use rm32_stm32::phase::COMP_PWM_LIVE;
                            // Resolve current effective mode, flip, force.
                            // Cycle 1(diode) -> 2(comp) -> 3(auto) -> 1
                            let cur = COMP_PWM_LIVE.load(Ordering::Relaxed);
                            let nxt = match cur {
                                1 => 2u8,
                                2 => 3,
                                _ => 1,
                            };
                            COMP_PWM_LIVE.store(nxt, Ordering::Relaxed);
                            rm32_stm32::dprintln!(
                                "[bench] drive: {} (live)",
                                match nxt {
                                    1 => "DIODE",
                                    2 => "COMPLEMENTARY",
                                    _ => "AUTO(comp iff interrupt-mode)",
                                }
                            );
                        }
                        UartCmd::AdcToggle => {
                            #[cfg(feature = "stm32l431")]
                            {
                                use core::sync::atomic::Ordering;
                                use rm32_stm32::mcu_l431::adc;
                                // Toggle 0 (normal scan) <-> 1 (paused,
                                // frozen readings).
                                let next = (adc::ADC_MODE.load(Ordering::Relaxed) + 1) % 2;
                                adc::ADC_MODE.store(next, Ordering::Relaxed);
                                rm32_stm32::dprintln!(
                                    "[bench] adc mode={} ({})",
                                    next,
                                    match next {
                                        1 => "PAUSED - readings frozen",
                                        _ => "normal sw scan",
                                    }
                                );
                            }
                            #[cfg(not(feature = "stm32l431"))]
                            rm32_stm32::dprintln!("[bench] adc toggle: L431 only");
                        }
                        UartCmd::WatchArm => {
                            #[cfg(all(feature = "stm32l431", feature = "zctrace"))]
                            unsafe {
                                let dhcsr = core::ptr::read_volatile(0xE000_EDF0 as *const u32);
                                if dhcsr & 1 == 0 {
                                    let addr = &rm32_stm32::phase::COMP_PWM_LIVE as *const _ as u32;
                                    let dcb = &*cortex_m::peripheral::DCB::PTR;
                                    dcb.demcr.modify(|v| v | (1 << 16));
                                    let dwt = &*cortex_m::peripheral::DWT::PTR;
                                    dwt.c[1].comp.write(addr);
                                    dwt.c[1].mask.write(0);
                                    dwt.c[1].function.write(0x6);
                                    rm32_stm32::dprintln!("[bench] DWT watch ARMED");
                                } else {
                                    rm32_stm32::dprintln!(
                                        "[bench] DWT watch refused: C_DEBUGEN set (power-cycle first)"
                                    );
                                }
                            }
                            #[cfg(not(all(feature = "stm32l431", feature = "zctrace")))]
                            rm32_stm32::dprintln!("[bench] watch: L431+zctrace only");
                        }
                        UartCmd::InjToggle => {
                            #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
                            {
                                rm32_stm32::mcu_l431::adc::arm_injected_current();
                                rm32_stm32::dprintln!("[bench] injected current ARMED");
                            }
                            #[cfg(not(all(feature = "benchuart", feature = "stm32l431")))]
                            rm32_stm32::dprintln!("[bench] injected: L431 bench only");
                        }
                        UartCmd::GeckoGrab => {
                            #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
                            {
                                let head = rm32_stm32::mcu_l431::adc::gecko_capture();
                                rm32_stm32::dprintln!(
                                    "gecko start={} n=2048 vbat={}",
                                    head,
                                    shared.battery_voltage()
                                );
                                for line in 0..128usize {
                                    use core::fmt::Write as _;
                                    let mut s = heapless::String::<96>::new();
                                    for k in 0..16usize {
                                        let v =
                                            rm32_stm32::mcu_l431::adc::gecko_word(line * 16 + k);
                                        let _ = write!(
                                            s,
                                            "{:04x}{}",
                                            v,
                                            if k == 15 { "" } else { " " }
                                        );
                                    }
                                    rm32_stm32::dprintln!("{}", s.as_str());
                                }
                                rm32_stm32::dprintln!("gecko end");
                            }
                            #[cfg(not(all(feature = "benchuart", feature = "stm32l431")))]
                            rm32_stm32::dprintln!("[bench] gecko: L431 bench only");
                        }
                        UartCmd::RecorderDump => {
                            #[cfg(feature = "stm32l431")]
                            dump_recorder();
                            #[cfg(not(feature = "stm32l431"))]
                            rm32_stm32::dprintln!("[bench] recorder: L431 only");
                        }
                        // Config verbs — same semantics as the PA0
                        // soft-UART dispatcher (A4 ring + save flag).
                        UartCmd::ConfigOffset(n) => {
                            bench_cfg_offset = n.min(255) as u8;
                            rm32_stm32::dprintln!("[cfg] offset={}", bench_cfg_offset);
                        }
                        UartCmd::ConfigWrite(v) => {
                            // Same boot-byte guard as the PA0 dispatcher.
                            if bench_cfg_offset < 3 {
                                rm32_stm32::dprintln!(
                                    "[cfg] REFUSED offset {} (<3)",
                                    bench_cfg_offset
                                );
                            } else {
                                shared.push_config_write(bench_cfg_offset, v.min(255) as u8);
                                rm32_stm32::dprintln!(
                                    "[cfg] write [{}]={} (ring)",
                                    bench_cfg_offset,
                                    v.min(255)
                                );
                            }
                        }
                        UartCmd::ConfigDump => {
                            #[cfg(feature = "stm32l431")]
                            dump_eeprom();
                            #[cfg(not(feature = "stm32l431"))]
                            rm32_stm32::dprintln!("[bench] eeprom dump: L431 only");
                        }
                        UartCmd::SaveConfig => {
                            shared.set_save_settings_flag(true);
                            rm32_stm32::dprintln!("[cfg] save requested");
                        }
                        UartCmd::HistDump => {
                            #[cfg(all(
                                feature = "benchuart",
                                any(feature = "stm32l431", feature = "stm32g431")
                            ))]
                            {
                                use core::fmt::Write as _;
                                let now =
                                    unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
                                rm32_stm32::dprintln!("HIST cyc={} shift=8 nbins=16", now);
                                let names = ["tim6clean", "tim16", "comp", "tim6preempt"];
                                for site in 0..4usize {
                                    let (bins, sum, cnt) = rm32_stm32::bench_hist::snapshot(site);
                                    let mut s = heapless::String::<224>::new();
                                    let _ = write!(s, "{} cnt={} sum={}", names[site], cnt, sum);
                                    for b in bins.iter() {
                                        let _ = write!(s, " {}", b);
                                    }
                                    rm32_stm32::dprintln!("{}", s.as_str());
                                    #[cfg(feature = "debuguart")]
                                    rm32_stm32::debug_uart::flush();
                                }
                                #[cfg(feature = "stm32l431")]
                                {
                                    use core::sync::atomic::Ordering;
                                    let ce = rm32_stm32::mcu_l431::interrupts::LEAN_COMP_ENTRIES
                                        .swap(0, Ordering::Relaxed);
                                    let cm = rm32_stm32::mcu_l431::interrupts::LEAN_COMMS
                                        .swap(0, Ordering::Relaxed);
                                    rm32_stm32::dprintln!(
                                        "HIST lean_comp_entries={} lean_comms={}",
                                        ce,
                                        cm
                                    );
                                }
                                rm32_stm32::dprintln!("HIST END");
                                rm32_stm32::bench_hist::reset();
                            }
                            #[cfg(not(all(
                                feature = "benchuart",
                                any(feature = "stm32l431", feature = "stm32g431")
                            )))]
                            rm32_stm32::dprintln!("[bench] hist: M4 bench only");
                        }
                        UartCmd::WaxDump => {
                            #[cfg(all(feature = "benchuart", feature = "stm32l431"))]
                            {
                                use core::fmt::Write as _;
                                let h = rm32_stm32::mcu_l431::adc::wax_head();
                                rm32_stm32::dprintln!(
                                    "WX n=1024 head={} ci={} arr={} frozen={}",
                                    h,
                                    shared.commutation_interval(),
                                    shared.tim1_arr(),
                                    rm32_stm32::mcu_l431::adc::wax_frozen() as u8
                                );
                                for line in 0..256usize {
                                    let mut s = heapless::String::<96>::new();
                                    for k in 0..4usize {
                                        let (a, b, pos, t1s) =
                                            rm32_stm32::mcu_l431::adc::wax_read(line * 4 + k);
                                        let _ = write!(
                                            s,
                                            "{:04x} {:04x} {:04x} {:04x}",
                                            a, b, pos, t1s
                                        );
                                        if k != 3 {
                                            let _ = write!(s, "  ");
                                        }
                                    }
                                    rm32_stm32::dprintln!("{}", s.as_str());
                                    // Pace the ~20 KB dump: without a
                                    // flush per line the TX ring drops
                                    // ~25% of records.
                                    #[cfg(feature = "debuguart")]
                                    rm32_stm32::debug_uart::flush();
                                }
                                rm32_stm32::dprintln!("WX END");
                                rm32_stm32::mcu_l431::adc::wax_rearm();
                            }
                            #[cfg(not(all(feature = "benchuart", feature = "stm32l431")))]
                            rm32_stm32::dprintln!("[bench] wax: L431 bench only");
                        }
                        UartCmd::BbDump => {
                            #[cfg(feature = "blackbox")]
                            {
                                let n = rm32_stm32::bench_bb::dump(|bytes| {
                                    if let Ok(s) = core::str::from_utf8(bytes) {
                                        rm32_stm32::debug_uart::write_str(s);
                                    }
                                });
                                rm32_stm32::dprintln!("[bench] bb dump: {} events", n);
                            }
                            #[cfg(not(feature = "blackbox"))]
                            rm32_stm32::dprintln!(
                                "[bench] blackbox: build without 'blackbox' feature"
                            );
                        }
                    }
                }
            }
            // A latched safety kill outranks any commanded throttle.
            if bench_guard.latched().is_some() {
                bench_throttle = 0;
            }
            // ZC-trace drain: up to 3 records per pass onto the bench wire
            // (binary, interleaved with the text log — the capture script
            // resyncs on the 5B A9 marker).
            #[cfg(feature = "zctrace")]
            if rm32_stm32::bench_zct::enabled() {
                rm32_stm32::bench_zct::drain(rm32_stm32::debug_uart::write_byte);
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

        // Arming feedback: LED + the A3 tone channel (arming tune stepped
        // by the 20 kHz tick's ToneScheduler — no HAL access needed here).
        if main_state.just_armed {
            shared.set_tone_request(rm32::tone::TONE_ARMED);
            if BOARD.has_led {
                led.set_status(LedStatus::Armed);
            }
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
        // BENCH: suppressed under benchuart — the bench has no DShot source,
        // so the timeout would bounce the chip every idle 2 s. Print
        // rate-limited (every 200k iters) so the UART doesn't choke main.
        if main_state.needs_reset {
            #[cfg(feature = "benchuart")]
            {
                main_state.needs_reset = false;
                if log_counter.is_multiple_of(200_000) {
                    rm32_stm32::dprintln!("[rm32] signal_timeout sticky (RESET SUPPRESSED)");
                }
            }
            #[cfg(not(feature = "benchuart"))]
            {
                // Last-words diagnostic: what the input pipeline saw this
                // life. Main context (prints in ISRs are forbidden).
                #[cfg(feature = "stm32l431")]
                let tim15_psc = unsafe { (*stm32l4xx_hal::pac::TIM15::PTR).psc.read().bits() };
                #[cfg(not(feature = "stm32l431"))]
                let tim15_psc = 0u32;
                rm32_stm32::dprintln!(
                    "[rm32] signal_timeout RESET: psc={} proto={}/{}/{} armed={} sig_to={} crc_pass={} crc_fail={} hi_pin={} newinput={} in_set={}",
                    tim15_psc,
                    shared.dshot() as u8,
                    shared.servo_pwm() as u8,
                    shared.dshot_telemetry() as u8,
                    shared.armed() as u8,
                    shared.signal_timeout(),
                    shared.dbg_crc_pass(),
                    shared.dbg_crc_fail(),
                    shared.dbg_high_pin_n(),
                    shared.newinput(),
                    shared.input_set() as u8,
                );
                // Frame-history autopsy: raw capture deltas for the last
                // frames this life (pass and fail) — protocol-rate and
                // alignment forensics for the 97%-fail class.
                #[cfg(feature = "debuguart")]
                for snap in rm32_stm32::dbg_frame_history::take().iter() {
                    rm32_stm32::dprintln!(
                        "[snap n={} pass={}] d={} {} {} {} {} {} {}",
                        snap.n,
                        snap.crc_pass as u8,
                        (snap.buf[1] as u16).wrapping_sub(snap.buf[0] as u16),
                        (snap.buf[2] as u16).wrapping_sub(snap.buf[1] as u16),
                        (snap.buf[3] as u16).wrapping_sub(snap.buf[2] as u16),
                        (snap.buf[4] as u16).wrapping_sub(snap.buf[3] as u16),
                        (snap.buf[5] as u16).wrapping_sub(snap.buf[4] as u16),
                        (snap.buf[6] as u16).wrapping_sub(snap.buf[5] as u16),
                        (snap.buf[7] as u16).wrapping_sub(snap.buf[6] as u16),
                    );
                }
                #[cfg(feature = "debuguart")]
                rm32_stm32::debug_uart::flush();
                sys.reset();
            }
        }

        // PA0 soft-UART RX drain + dispatch through the shared
        // bench_input vocabulary. Throttle verbs are IGNORED here — in
        // DSHOT/BF mode throttle ownership stays on the signal wire;
        // this channel is for config surgery, dumps and the kill verb.
        #[cfg(all(feature = "debuguart", feature = "stm32l431"))]
        {
            use rm32::bench_input::UartCmd;
            let su_now = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
            for _ in 0..8 {
                let Some(b) = softuart_rx.dequeue() else {
                    break;
                };
                su_last = b;
                su_n = su_n.wrapping_add(1);
                let Some(cmd) = su_parser.step(b) else {
                    continue;
                };
                if cmd == UartCmd::Kill {
                    // Safety verb: never deferred.
                    su_execute(cmd, &mut su_cfg_offset);
                } else {
                    // Defer ~150 ms (12M cycles @80 MHz) so the reply
                    // transmits after the host's baud switch-back. A
                    // second command displaces the slot by running the
                    // first immediately (hosts send sequentially).
                    if let Some((old, _)) = su_pending.take() {
                        su_execute(old, &mut su_cfg_offset);
                    }
                    su_pending = Some((cmd, su_now.wrapping_add(12_000_000)));
                }
            }
            if let Some((cmd, due)) = su_pending {
                if su_now.wrapping_sub(due) < u32::MAX / 2 {
                    su_pending = None;
                    su_execute(cmd, &mut su_cfg_offset);
                }
            }
        }

        // Item-7 A/B: deliberate PB6 print traffic WHILE RUNNING (cmd 42
        // lever). ~60 chars every 2048 iters — comparable to the old 'i'
        // polling that built the wall. Main context.
        #[cfg(feature = "debuguart")]
        if log_counter % 2048 == 0
            && shared.running()
            && rm32_stm32::isr_handlers::PRINT_BLAST.load(core::sync::atomic::Ordering::Relaxed)
        {
            rm32_stm32::dprintln!(
                "[blast] ci={} zc={} filler=XXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
                shared.commutation_interval(),
                shared.zero_crosses()
            );
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

/// Dump the onboard flight recorder (0.5 s ci/mA/mV samples) — shared
/// by the benchuart 'B' arm and the PA0 soft-UART dispatcher.
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
fn dump_recorder() {
    use core::sync::atomic::Ordering;
    use rm32_stm32::isr_handlers as ih;
    let head = ih::SR_HEAD.load(Ordering::Relaxed) as usize;
    let n = head.min(ih::SR_N);
    let start = if head > ih::SR_N { head % ih::SR_N } else { 0 };
    rm32_stm32::dprintln!("SR n={} dt_ms=500", n);
    for k in 0..n {
        let i = (start + k) % ih::SR_N;
        rm32_stm32::dprintln!(
            "SR {} {} {}",
            ih::SR_CI[i].load(Ordering::Relaxed),
            ih::SR_MA[i].load(Ordering::Relaxed),
            ih::SR_MV[i].load(Ordering::Relaxed)
        );
        if k % 8 == 7 {
            rm32_stm32::debug_uart::flush();
        }
    }
    rm32_stm32::dprintln!("SR END");
}

/// Execute one PA0 soft-UART command (deferred dispatch — see the
/// drain loop). Throttle verbs and benchuart-era facilities report
/// as ignored; config verbs ride the A4 ring + save flag.
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
fn su_execute(cmd: rm32::bench_input::UartCmd, cfg_offset: &mut u8) {
    use rm32::bench_input::UartCmd;
    let shared = isr::shared();
    match cmd {
        UartCmd::ConfigOffset(n) => {
            *cfg_offset = n.min(255) as u8;
            rm32_stm32::dprintln!("[cfg] offset={}", *cfg_offset);
        }
        UartCmd::ConfigWrite(v) => {
            // Refuse the boot-enable/layout bytes: a mis-decoded offset
            // once landed a stray write at [0], and a bootloader that
            // checks boot-enable bricks the app jump on the next reset
            // (2026-08-01 incident — repaired over SWD).
            if *cfg_offset < 3 {
                rm32_stm32::dprintln!("[cfg] REFUSED offset {} (<3)", *cfg_offset);
            } else {
                // A4 write-through ring: run_tick drains into
                // main.config — the same plumbing Configurator / DSHOT
                // programming use.
                shared.push_config_write(*cfg_offset, v.min(255) as u8);
                rm32_stm32::dprintln!("[cfg] write [{}]={} (ring)", *cfg_offset, v.min(255));
            }
        }
        UartCmd::ConfigDump => dump_eeprom(),
        UartCmd::SaveConfig => {
            // Same save path as DSHOT cmd 12.
            shared.set_save_settings_flag(true);
            rm32_stm32::dprintln!("[cfg] save requested");
        }
        UartCmd::Kill => {
            shared.request_isr_action(rm32::shared_comm::IsrAction::AllOff);
            rm32_stm32::dprintln!("[su] KILL: all off");
        }
        UartCmd::Info => {
            rm32_stm32::dprintln!(
                "[i mode={:?} newinput={} adj={} fwd={} ecom={} ci={} duty={} zc={} vbat_mv={}]",
                shared.motor_mode(),
                shared.newinput(),
                shared.adjusted_input(),
                shared.forward() as u8,
                shared.e_com_time(),
                shared.commutation_interval(),
                shared.duty_cycle(),
                shared.zero_crosses(),
                shared.battery_voltage()
            );
        }
        UartCmd::RecorderDump => dump_recorder(),
        other => {
            rm32_stm32::dprintln!("[su] ignored: {:?}", other);
        }
    }
}

/// Hex-dump the PERSISTED EEPROM config page (raw `EepromConfig`
/// bytes at 0x0800F800) — shared by both UART dispatchers' 'c' verb.
#[cfg(all(feature = "debuguart", feature = "stm32l431"))]
fn dump_eeprom() {
    let len = core::mem::size_of::<rm32::config::EepromConfig>();
    let eep = unsafe { core::slice::from_raw_parts(0x0800_F800 as *const u8, len) };
    for (row, chunk) in eep.chunks(16).enumerate() {
        let mut line = heapless::String::<64>::new();
        for b in chunk {
            let _ = core::fmt::Write::write_fmt(&mut line, format_args!("{:02x} ", b));
        }
        rm32_stm32::dprintln!("[eep {:02}] {}", row * 16, line.as_str());
        rm32_stm32::debug_uart::flush();
    }
}

/// Self-hosted watchpoint catcher. The DWT write-comparator on
/// phase::COMP_PWM_LIVE raises DebugMonitor; the exception frame sits
/// above this handler's own frame on MSP. Rather than fight prologue
/// offsets, scan upward for the first two flash-range words — the
/// stacked LR (thumb bit set) and PC of the writer. One-shot.
#[cfg(all(feature = "benchuart", feature = "stm32l431", feature = "zctrace"))]
#[cortex_m_rt::exception]
fn DebugMonitor() {
    let msp = cortex_m::register::msp::read();
    let mut found = [0u32; 2];
    let mut n = 0;
    for off in (0..96u32).step_by(4) {
        let v = unsafe { core::ptr::read_volatile((msp + off) as *const u32) };
        if (0x0800_0000..0x0801_0000).contains(&(v & !1)) {
            found[n] = v;
            n += 1;
            if n == 2 {
                break;
            }
        }
    }
    rm32_stm32::edge_probe::watch_store(found.get(1).copied().unwrap_or(0), found[0]);
}
