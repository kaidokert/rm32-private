use crate::signed_calc::SignedCalc;

pub trait FilterGeneric<T: SignedCalc> {
    type State;

    fn filter(&self, value: T, state: &mut Self::State) -> T;
}

/// A generic trait for filters that process a stream of i32 values.
pub trait Filter {
    fn filter(&mut self, value: i32) -> i32;
}

pub struct NoOpFilter;

impl Filter for NoOpFilter {
    fn filter(&mut self, value: i32) -> i32 {
        value
    }
}

pub struct NoOpFilterGeneric;

impl<T: SignedCalc> FilterGeneric<T> for NoOpFilterGeneric {
    type State = T;

    fn filter(&self, value: T, _state: &mut Self::State) -> T {
        value
    }
}
