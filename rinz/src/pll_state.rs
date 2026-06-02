use crate::pi_controller_state::PIControllerState;
use crate::signed_calc::SignedCalc;

pub struct PLLState<T: SignedCalc> {
    pub pi: PIControllerState<T>,
    pub phase: T,
    pub frequency: T,
}

impl<T: SignedCalc> PLLState<T> {
    pub fn new(phase: T, frequency: T) -> Self {
        Self {
            pi: PIControllerState::from(T::zero(), T::zero()),
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
