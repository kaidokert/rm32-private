use crate::pi_controller_state::PIControllerState;
use crate::signed_calc::SignedCalc;

/// State for the PLL frequency estimator.
///
/// Phase is not tracked here — it belongs in `PhaseAccumulatorState`, which
/// is driven by a separate fixed-rate integrator outside this block.
/// `pi.integral` is the authoritative frequency state; `frequency` is a
/// read-out cache of the last `update()` return value.
pub struct PLLState<T: SignedCalc> {
    pub pi: PIControllerState<T>,
    pub frequency: T,
}

impl<T: SignedCalc> PLLState<T> {
    /// Construct with a known starting frequency.
    ///
    /// `pi.integral` is initialized to `frequency` so that `update(0)` holds
    /// the starting frequency rather than decaying toward zero.
    pub fn new(frequency: T) -> Self {
        Self {
            pi: PIControllerState::from(frequency, T::zero()),
            frequency,
        }
    }
}

impl<T: SignedCalc> Default for PLLState<T> {
    fn default() -> Self {
        Self::new(T::zero())
    }
}
