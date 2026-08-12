//! Unified system tick — single entry point for main-loop pipeline.
//!
//! Both the firmware (`main.rs`) and the test harness (`harness.rs`)
//! call these functions. This ensures the control pipeline is identical
//! in both contexts, eliminating the "harness vs firmware" divergence
//! that caused bugs across multiple review rounds.

use crate::control::input::{self, InputState};
use crate::hal::{Adc, TelemetryUart};
use crate::main_state::MainState;
use crate::shared_state::SharedState;
use crate::sine::PhasePositions;
use embedded_hal::digital::OutputPin;

/// Main-loop system tick state.
///
/// Owns the `InputState` and `PhasePositions` (sine mode) that were
/// previously duplicated or missing between harness and firmware.
pub struct SystemTick {
    pub input_state: InputState,
    sine_positions: PhasePositions,
}

impl SystemTick {
    pub fn new() -> Self {
        Self {
            input_state: InputState::new(),
            sine_positions: PhasePositions::new(),
        }
    }

    /// Run input processing pipeline.
    ///
    /// Call this BEFORE the ISR tick (harness) or independently (firmware,
    /// where the ISR tick runs in the actual interrupt).
    pub fn tick_input<LED: OutputPin>(&mut self, shared: &SharedState, main: &mut MainState<LED>) {
        // Recompute input mode from config + detected protocol each tick.
        // Cheap (a few comparisons) and ensures mode stays in sync with config.
        self.input_state.mode =
            crate::input_mapping::InputMode::from_config(&main.config, shared.dshot());
        input::process_input(
            shared,
            &main.config,
            &mut main.protection,
            &mut self.input_state,
        );
    }

    /// Run main-loop pipeline.
    ///
    /// Call this AFTER the ISR tick.
    pub fn tick_main<LED: OutputPin>(
        &self,
        shared: &SharedState,
        main: &mut MainState<LED>,
        adc: &mut dyn Adc,
        telem: &mut dyn TelemetryUart,
    ) {
        main.tick(shared, adc, telem);
    }

    /// Sync ISR→main one-shot flags via SharedState atomics.
    ///
    /// ISR publishes `desync_check_pending` to SharedState; main reads and
    /// clears it here. No `with_isr_state` needed — all through atomics.
    pub fn sync_isr_to_main<LED: OutputPin>(
        &self,
        shared: &SharedState,
        main: &mut MainState<LED>,
    ) {
        if shared.take_desync_check_pending() {
            main.set_desync_check(true);
        }
    }

    /// Canonical main-loop tick with platform callback.
    ///
    /// Captures the exact orchestration order that both harness and firmware
    /// must follow. The single `isr_and_sync` closure handles:
    /// 1. Running the ISR tick (inline for harness, no-op for firmware)
    /// 2. Syncing ISR→main one-shot flags (calls `sync_isr_to_main`)
    ///
    /// Using a single closure avoids borrow conflicts between ISR state
    /// (commutation, bemf, etc.) and the sync step that reads commutation.
    /// Process sine mode stepping.
    ///
    /// Returns the SineStepResult and PWM values. The caller applies
    /// PWM output and handles changeover via platform-specific HAL calls.
    pub fn tick_sine(
        &mut self,
        shared: &SharedState,
        config: &crate::config::EepromConfig,
        dead_time: i16,
        tim1_autoreload: u16,
    ) -> Option<(crate::sine::SineStepResult, (u16, u16, u16))> {
        if !shared.stepper_sine() {
            return None;
        }
        Some(crate::sine::sine_step(
            &mut self.sine_positions,
            shared.newinput(),
            shared.armed(),
            shared.forward(),
            config.motor_poles,
            crate::constants::SINE_CHANGEOVER_STEP,
            dead_time,
            tim1_autoreload,
            config.sine_mode_power,
        ))
    }

    /// Apply sine changeover state transitions.
    ///
    /// Called when `tick_sine` returns `Changeover`. Sets shared state
    /// and main-loop timing. The caller handles ISR HAL calls
    /// (com_step, generate_update_event, etc.) via platform-specific code.
    pub fn apply_sine_changeover<LED: OutputPin>(
        &mut self,
        shared: &SharedState,
        main: &mut MainState<LED>,
        commutation_interval: u32,
        step: u8,
    ) {
        shared.set_commutation_interval(commutation_interval);
        shared.set_zero_crosses(20);
        shared.set_prop_brake_active(false);
        main.timing_mut().set_average_interval(commutation_interval);
        main.timing_mut()
            .set_last_average_interval(commutation_interval);
        shared.set_changeover_step(step);
        shared.transition(crate::motor_mode::MotorEvent::ExitSine);
    }

    /// Handle sine mode idle (throttle=0 or !armed) brake logic.
    ///
    /// Matches C main.c lines 2258-2282. Returns true if prop_brake_active
    /// should be set (brake_on_stop==1 with sufficient drag_brake_strength).
    pub fn handle_sine_idle(config: &crate::config::EepromConfig, tim1_arr: u16) -> bool {
        if config.brake_on_stop == 1 {
            let prop_brake_duty = config.drag_brake_strength as u32 * 200;
            let tim1_arr = tim1_arr as u32;
            let scaled = ((prop_brake_duty * tim1_arr) / 2000).min(tim1_arr);
            let adjusted = tim1_arr - scaled;
            adjusted >= 100 // below 100 → fullBrake instead (handled by caller)
        } else {
            false
        }
    }

    pub fn run_tick<LED: OutputPin>(
        &mut self,
        shared: &SharedState,
        main: &mut MainState<LED>,
        adc: &mut dyn Adc,
        telem: &mut dyn TelemetryUart,
        isr_tick: impl FnOnce(),
    ) {
        // A4: drain ISR-side config byte writes into main's copy BEFORE
        // any save-settings check this pass — the ISR command processor
        // mutates its own EepromConfig; without this, save persisted a
        // stale main copy (Configurator/DSHOT-written settings lost).
        while let Some((off, val)) = shared.pop_config_write() {
            if (off as usize) < main.config.as_bytes().len() {
                main.config.as_bytes_mut()[off as usize] = val;
            }
        }
        // 1. Input processing
        self.tick_input(shared, main);

        // 2. ISR tick (harness runs inline, firmware is a no-op — ISR runs async)
        isr_tick();

        // 3. Sync ISR→main flags via SharedState atomics (no with_isr_state)
        self.sync_isr_to_main(shared, main);

        // 4. Main-loop pipeline
        self.tick_main(shared, main, adc, telem);
    }
}

impl Default for SystemTick {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::BoardConfig;
    use crate::config::EepromConfig;
    use crate::main_state::{ChipParams, MainState};
    use crate::motor_mode::MotorEvent;
    use crate::sine::SineStepResult;

    struct MockAdc;
    impl MockAdc {
        fn new() -> Self {
            Self
        }
    }
    impl crate::hal::Adc for MockAdc {
        fn start_conversion(&mut self) {}
        fn raw_voltage(&self) -> u16 {
            0
        }
        fn raw_current(&self) -> u16 {
            0
        }
        fn raw_temperature(&self) -> u16 {
            0
        }
        fn calc_temperature(&self, _: u16) -> crate::units::DegreesCelsius {
            crate::units::DegreesCelsius(25)
        }
    }

    struct MockTelem;
    impl crate::hal::TelemetryUart for MockTelem {
        fn send_dma(&mut self, _: &[u8]) {}
    }

    fn make_main() -> MainState {
        MainState::new(
            &BoardConfig::DEFAULT,
            ChipParams {
                timer1_max_arr: 1999,
                cpu_mhz: 64,
            },
        )
    }

    #[test]
    fn config_write_through_reaches_main_copy() {
        // A4 regression: ISR-side config byte writes published via the
        // SPSC ring must land in main's copy during run_tick, BEFORE any
        // save-settings action would persist it.
        let shared = SharedState::new();
        let mut main = make_main();
        let off = core::mem::offset_of!(crate::config::EepromConfig, dir_reversed) as u8;
        shared.push_config_write(off, 1);
        shared.push_config_write(3, 77); // arbitrary programming byte
        let mut sys = SystemTick::new();
        sys.run_tick(
            &shared,
            &mut main,
            &mut MockAdc::new(),
            &mut MockTelem,
            || {},
        );
        assert_eq!(main.config.dir_reversed, 1);
        assert_eq!(main.config.as_bytes()[3], 77);
        // Ring drained.
        assert!(shared.pop_config_write().is_none());
    }

    #[test]
    fn sync_isr_to_main_transfers_desync_check() {
        let sys = SystemTick::new();
        let shared = SharedState::new();
        let mut main = make_main();

        shared.set_desync_check_pending(true);
        assert!(!main.desync_check(), "main starts clear");
        sys.sync_isr_to_main(&shared, &mut main);
        assert!(
            main.desync_check(),
            "main.desync_check should be set after transfer"
        );
        assert!(
            !shared.desync_check_pending(),
            "shared.desync_check_pending should be cleared after transfer"
        );
    }

    #[test]
    fn sync_isr_to_main_noop_when_flag_clear() {
        let sys = SystemTick::new();
        let shared = SharedState::new();
        let mut main = make_main();
        // Pre-set main.desync_check; shared flag is clear → main should be untouched.
        main.set_desync_check(true);
        sys.sync_isr_to_main(&shared, &mut main);
        assert!(
            main.desync_check(),
            "main.desync_check unchanged when shared flag is false"
        );
    }

    #[test]
    fn tick_sine_runs_only_in_sine_mode() {
        let shared = SharedState::new();
        let mut system = SystemTick::new();
        let config = EepromConfig::default();

        assert!(system.tick_sine(&shared, &config, 60, 1999).is_none());

        shared.transition(MotorEvent::Arm);
        shared.transition(MotorEvent::EnterSine);
        shared.set_newinput(crate::constants::SINE_CHANGEOVER_THROTTLE + 1);

        let Some((
            SineStepResult::Changeover {
                commutation_interval,
                step,
            },
            pwm,
        )) = system.tick_sine(&shared, &config, 60, 1999)
        else {
            panic!("expected sine changeover");
        };

        assert_eq!(commutation_interval, 9000);
        assert_eq!(step, crate::constants::SINE_CHANGEOVER_STEP);
        assert!(pwm.0 > 0 || pwm.1 > 0 || pwm.2 > 0);
    }

    #[test]
    fn apply_sine_changeover_publishes_handoff_before_exit() {
        let shared = SharedState::new();
        let mut main = make_main();
        let mut system = SystemTick::new();

        shared.transition(MotorEvent::Arm);
        shared.transition(MotorEvent::EnterSine);
        shared.set_prop_brake_active(true);

        system.apply_sine_changeover(&shared, &mut main, 9000, 5);

        assert!(shared.old_routine());
        assert_eq!(shared.commutation_interval(), 9000);
        assert_eq!(shared.zero_crosses(), 20);
        assert_eq!(shared.changeover_step(), 5);
        assert_eq!(main.timing().average_interval(), 9000);
        assert!(!shared.prop_brake_active());
    }

    #[test]
    fn handle_sine_idle_applies_brake_policy_without_underflow() {
        let mut config = EepromConfig::default();

        assert!(!SystemTick::handle_sine_idle(&config, 1999));

        config.brake_on_stop = 1;
        config.drag_brake_strength = 5;
        assert!(SystemTick::handle_sine_idle(&config, 1999));

        config.drag_brake_strength = u8::MAX;
        assert!(!SystemTick::handle_sine_idle(&config, 1999));
    }
}
