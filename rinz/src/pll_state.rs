use crate::pi_controller_state::PIControllerState;
use crate::signed_calc::SignedCalc;

pub struct PLLState<T: SignedCalc> {
    pub pi: PIControllerState<T>,
    pub phase: T,
    pub frequency: T,
}

impl<T: SignedCalc> PLLState<T> {
    /// Construct with a known starting frequency.
    ///
    /// `pi.integral` is initialized to `frequency` so that `update(0)` holds
    /// the starting frequency rather than decaying toward zero. `frequency` is
    /// the authoritative initial state; the `frequency` field is a readout
    /// cache and is kept consistent with it.
    pub fn new(phase: T, frequency: T) -> Self {
        Self {
            pi: PIControllerState::from(frequency, T::zero()),
            phase,
            frequency,
        }
    }
}

impl<T: SignedCalc> Default for PLLState<T> {
    fn default() -> Self {
        Self::new(T::zero(), T::zero())
    }
}
