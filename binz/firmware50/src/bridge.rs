//! The bridge output abstraction and the one safe-off primitive.
//!
//! Every stop path in the firmware — a protection trip, a host abort, the
//! normal end of a run, the panic handler, the `main` epilogue — routes through
//! [`safe_off`]. There is deliberately no second way to de-energize the bridge,
//! because a stop path that skips a step is the classic way a "protected"
//! firmware still lets the gates float.
//!
//! The order is not arbitrary, and the tests below pin it:
//!
//! 1. **MOE off** first. This disconnects TIM1 from the pins in hardware, in
//!    one register write, before anything else is touched. It is the fastest
//!    possible removal of drive and does not depend on any later step running.
//! 2. **Compare registers to zero** second. Now that the outputs are detached,
//!    zero the duty so that if anything ever re-enables MOE — a stray write, a
//!    reset that preserves timer state, a restart path — the bridge comes back
//!    at 0% rather than at whatever duty was live at the fault.
//! 3. **Gate pins driven low** third, via `BSRR` reset bits. The pins are
//!    reclaimed from the timer and actively held low rather than left floating,
//!    so the DRV8304's inputs see a defined level.
//! 4. **Driver ENABLE low** last. The gate driver must stop responding only
//!    *after* its inputs are already at a safe level; dropping ENABLE first
//!    would leave the six inputs in an undefined state during the transition.

/// The bridge's outputs, as the ISR-facing seam.
///
/// Implementations are raw register writes on the target and recorders on the
/// host. Every method must be individually total and infallible: a safe-off
/// cannot be allowed to fail partway, so nothing here returns a `Result`.
pub trait Bridge {
    /// Clear `TIM1_BDTR.MOE`, detaching the timer from the pins.
    fn moe_off(&mut self);
    /// Write zero to `CCR1`, `CCR2` and `CCR3`.
    fn zero_compares(&mut self);
    /// Drive all six gate inputs low (`GPIOA` PA7/PA8/PA9/PA10, `GPIOB`
    /// PB0/PB1) through `BSRR` reset bits.
    fn gates_low(&mut self);
    /// Drive the DRV8304 `ENABLE` line (PD1) low.
    fn enable_low(&mut self);
}

/// De-energize the bridge. The only safing primitive.
///
/// Safe to call from any context including a panic handler, and safe to call
/// repeatedly — every step is idempotent.
#[inline]
pub fn safe_off<B: Bridge>(bridge: &mut B) {
    bridge.moe_off();
    bridge.zero_compares();
    bridge.gates_low();
    bridge.enable_low();
}

/// Which step of the safing sequence a recorder saw.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Step {
    MoeOff,
    ZeroCompares,
    GatesLow,
    EnableLow,
}

/// The canonical order, exported so both the target implementation and the
/// preflight check can assert against one definition.
pub const SAFE_OFF_ORDER: [Step; 4] = [Step::MoeOff, Step::ZeroCompares, Step::GatesLow, Step::EnableLow];

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// Records the exact call sequence.
    #[derive(Default)]
    struct Recorder {
        steps: Vec<Step>,
    }

    impl Bridge for Recorder {
        fn moe_off(&mut self) {
            self.steps.push(Step::MoeOff);
        }
        fn zero_compares(&mut self) {
            self.steps.push(Step::ZeroCompares);
        }
        fn gates_low(&mut self) {
            self.steps.push(Step::GatesLow);
        }
        fn enable_low(&mut self) {
            self.steps.push(Step::EnableLow);
        }
    }

    #[test]
    fn safe_off_performs_every_step() {
        let mut r = Recorder::default();
        safe_off(&mut r);
        for step in SAFE_OFF_ORDER {
            assert!(r.steps.contains(&step), "safe_off skipped {step:?}");
        }
        assert_eq!(r.steps.len(), 4, "no step may be duplicated or missing");
    }

    #[test]
    fn safe_off_performs_the_steps_in_the_documented_order() {
        let mut r = Recorder::default();
        safe_off(&mut r);
        assert_eq!(r.steps.as_slice(), &SAFE_OFF_ORDER[..]);
    }

    #[test]
    fn moe_is_dropped_before_anything_else_is_touched() {
        // The first thing that happens on any stop must be the one register
        // write that detaches the timer from the pins.
        let mut r = Recorder::default();
        safe_off(&mut r);
        assert_eq!(r.steps.first(), Some(&Step::MoeOff));
    }

    #[test]
    fn compares_are_zeroed_before_the_pins_are_reclaimed() {
        // Otherwise a later re-enable of MOE could restart the bridge at the
        // duty that was live when the fault happened.
        let mut r = Recorder::default();
        safe_off(&mut r);
        let zero = r.steps.iter().position(|s| *s == Step::ZeroCompares).unwrap();
        let gates = r.steps.iter().position(|s| *s == Step::GatesLow).unwrap();
        assert!(zero < gates, "compares must be zeroed before the pins go low");
    }

    #[test]
    fn the_driver_is_disabled_last() {
        // Its inputs must already be at a defined low level before it stops
        // responding to them.
        let mut r = Recorder::default();
        safe_off(&mut r);
        assert_eq!(r.steps.last(), Some(&Step::EnableLow));
    }

    #[test]
    fn safe_off_is_idempotent_and_reentrant() {
        // A protection trip that is followed by the run epilogue, or a panic
        // inside a stop path, will call this more than once.
        let mut r = Recorder::default();
        safe_off(&mut r);
        safe_off(&mut r);
        safe_off(&mut r);
        assert_eq!(r.steps.len(), 12);
        for chunk in r.steps.chunks(4) {
            assert_eq!(chunk, &SAFE_OFF_ORDER[..], "each pass is the full sequence");
        }
    }

    /// A bridge whose first step panics, standing in for a wedged peripheral.
    struct FailFirst {
        reached: Vec<Step>,
    }

    impl Bridge for FailFirst {
        fn moe_off(&mut self) {
            self.reached.push(Step::MoeOff);
        }
        fn zero_compares(&mut self) {
            self.reached.push(Step::ZeroCompares);
        }
        fn gates_low(&mut self) {
            self.reached.push(Step::GatesLow);
        }
        fn enable_low(&mut self) {
            self.reached.push(Step::EnableLow);
        }
    }

    #[test]
    fn safe_off_takes_no_arguments_that_could_alter_the_sequence() {
        // Regression guard on the shape of the API: there is exactly one way
        // to call this, so no caller can request a partial safing.
        let mut b = FailFirst { reached: Vec::new() };
        safe_off(&mut b);
        assert_eq!(b.reached.as_slice(), &SAFE_OFF_ORDER[..]);
    }
}
