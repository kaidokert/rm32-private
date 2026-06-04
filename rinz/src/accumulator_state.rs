use crate::signed_calc::SignedCalc;

pub struct AccumulatorState<T: SignedCalc> {
    pub value: T,
}

impl<T: SignedCalc> AccumulatorState<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T: SignedCalc> Default for AccumulatorState<T> {
    fn default() -> Self {
        Self::new(T::zero())
    }
}
