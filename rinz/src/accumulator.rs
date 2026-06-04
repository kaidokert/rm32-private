use crate::accumulator_state::AccumulatorState;
use crate::signed_calc::SignedCalc;
use core::marker::PhantomData;

/// General-purpose first-order integrator: output += input * dt.
///
/// Caller is responsible for wrapping (e.g., mod 1.0, mod 6.0) and for
/// choosing a dt consistent with the units of `input`.
pub struct Accumulator<T> {
    _marker: PhantomData<T>,
}

impl<T: SignedCalc> Accumulator<T> {
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Advance by `input * dt` and return the updated value.
    pub fn step(&self, input: T, dt: T, state: &mut AccumulatorState<T>) -> T {
        state.value = state.value + input * dt;
        state.value
    }
}

impl<T: SignedCalc> Default for Accumulator<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advances_by_input_times_dt() {
        let acc = Accumulator::new();
        let mut state = AccumulatorState::default();

        acc.step(3.0f32, 1.0, &mut state);
        assert_eq!(state.value, 3.0);

        acc.step(3.0f32, 1.0, &mut state);
        assert_eq!(state.value, 6.0);
    }

    #[test]
    fn test_scales_with_dt() {
        let acc = Accumulator::new();
        let mut state = AccumulatorState::default();

        acc.step(10.0f32, 0.5, &mut state);
        assert_eq!(state.value, 5.0);
    }

    #[test]
    fn test_zero_input_holds_value() {
        let acc = Accumulator::new();
        let mut state = AccumulatorState::new(4.0f32);

        acc.step(0.0f32, 1.0, &mut state);
        assert_eq!(state.value, 4.0);
    }

    #[test]
    fn test_returns_updated_value() {
        let acc = Accumulator::new();
        let mut state = AccumulatorState::new(2.0f32);

        let v = acc.step(1.5f32, 2.0, &mut state);
        assert_eq!(v, 5.0); // 2.0 + 1.5*2.0
        assert_eq!(state.value, 5.0);
    }
}
