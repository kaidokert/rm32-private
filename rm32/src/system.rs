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
use embedded_hal::digital::OutputPin;

/// Main-loop system tick state.
///
/// Owns the `InputState` that was previously duplicated between
/// harness and firmware. Both call the same `tick_input()` and
/// `tick_main()` methods.
pub struct SystemTick {
    pub input_state: InputState,
}

impl SystemTick {
    pub fn new() -> Self {
        Self {
            input_state: InputState::new(),
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

    /// Sync per-cycle flags from ISR-owned `Commutation` into `MainState`.
    ///
    /// Call between the ISR tick and `tick_main()`. The ISR sets one-shot
    /// flags on `Commutation` (e.g. `desync_check` on each BEMF zero-cross at
    /// `commutation.rs:43,50`); the main loop consumes them on the next pass
    /// (e.g. desync detection in `MainState::tick`, mirroring AM32
    /// `Src/main.c:1969-1985`).
    ///
    /// This used to be inline in `harness.rs::do_tick` and missing from the
    /// firmware main loop, so blackbox tests passed but on real hardware
    /// stalled-motor false-sync went undetected — `MainState.desync_check`
    /// stayed permanently false because nobody transferred the flag from
    /// `Commutation`. Centralising the transfer here means both paths
    /// automatically pick up future ISR→main one-shots in the same place.
    pub fn sync_isr_to_main<LED: OutputPin>(
        &self,
        commutation: &mut crate::commutation::Commutation,
        main: &mut MainState<LED>,
    ) {
        if commutation.desync_check() {
            main.set_desync_check(true);
            commutation.set_desync_check(false);
        }
    }
}

impl SystemTick {
    /// Canonical main-loop tick with platform callback.
    ///
    /// Captures the exact orchestration order that both harness and firmware
    /// must follow. The single `isr_and_sync` closure handles:
    /// 1. Running the ISR tick (inline for harness, no-op for firmware)
    /// 2. Syncing ISR→main one-shot flags (calls `sync_isr_to_main`)
    ///
    /// Using a single closure avoids borrow conflicts between ISR state
    /// (commutation, bemf, etc.) and the sync step that reads commutation.
    pub fn run_tick<LED: OutputPin>(
        &mut self,
        shared: &SharedState,
        main: &mut MainState<LED>,
        adc: &mut dyn Adc,
        telem: &mut dyn TelemetryUart,
        isr_and_sync: impl FnOnce(&Self, &mut MainState<LED>),
    ) {
        // 1. Input processing
        self.tick_input(shared, main);

        // 2. ISR tick + sync ISR→main flags (platform-specific)
        isr_and_sync(self, main);

        // 3. Main-loop pipeline
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
    use crate::commutation::Commutation;
    use crate::main_state::{ChipParams, MainState};

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
    fn sync_isr_to_main_transfers_desync_check() {
        let sys = SystemTick::new();
        let mut commutation = Commutation::new();
        let mut main = make_main();

        commutation.set_desync_check(true);
        assert!(!main.desync_check(), "main starts clear");
        sys.sync_isr_to_main(&mut commutation, &mut main);
        assert!(
            main.desync_check(),
            "main.desync_check should be set after transfer"
        );
        assert!(
            !commutation.desync_check(),
            "commutation.desync_check should be cleared after transfer"
        );
    }

    #[test]
    fn sync_isr_to_main_noop_when_flag_clear() {
        let sys = SystemTick::new();
        let mut commutation = Commutation::new();
        let mut main = make_main();
        // Pre-set main.desync_check; commutation flag is clear → main should be untouched.
        main.set_desync_check(true);
        sys.sync_isr_to_main(&mut commutation, &mut main);
        assert!(
            main.desync_check(),
            "main.desync_check unchanged when commutation flag is false"
        );
    }
}
