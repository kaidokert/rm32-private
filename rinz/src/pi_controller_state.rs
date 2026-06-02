use crate::signed_calc::SignedCalc;

pub struct PIControllerState<T: SignedCalc> {
    pub integral: T,
    pub last_error: T,
}
impl<T: SignedCalc> PIControllerState<T> {
    pub const fn from(int: T, err: T) -> Self {
        Self {
            integral: int,
            last_error: err,
        }
    }
}
impl<T: SignedCalc> Default for PIControllerState<T> {
    fn default() -> Self {
        Self::from(T::zero(), T::zero())
    }
}
