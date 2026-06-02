use crate::algorithm_params::define_params;
use crate::pll_state::PLLState;
use crate::signed_calc::SignedCalc;

define_params!(PLLParams {
    kp: T,
    ki: T,
    freq_min: T,
    freq_max: T
});

pub struct PLL<T, P> {
    params: P,
    _marker: core::marker::PhantomData<T>,
}

impl<T: SignedCalc + Copy, P: PLLParamsProvider<T>> PLL<T, P> {
    pub const fn new(params: P) -> Self {
        Self {
            params,
            _marker: core::marker::PhantomData,
        }
    }

    /// Update the PLL with a new phase error measurement.
    ///
    /// phase_error = measured_zc_time − predicted_zc_time (state.phase).
    /// The loop filter (PI with anti-windup) converts phase error to a
    /// frequency estimate, which is then integrated into state.phase.
    ///
    /// Returns the updated frequency estimate.
    pub fn update(&self, phase_error: T, state: &mut PLLState<T>) -> T {
        self.params.with_snapshot(|p| {
            // Loop filter: PI on phase error, output is frequency
            let pterm = p.kp * phase_error;
            let i_candidate = state.pi.integral + p.ki * phase_error;
            let u_temp = pterm + i_candidate;

            let zero = T::zero();
            let mut int_ok = true;
            let mut freq = u_temp;

            if u_temp > p.freq_max {
                freq = p.freq_max;
                if phase_error > zero {
                    int_ok = false;
                }
            } else if u_temp < p.freq_min {
                freq = p.freq_min;
                if phase_error < zero {
                    int_ok = false;
                }
            }

            if int_ok {
                state.pi.integral = i_candidate;
            }
            state.pi.last_error = phase_error;

            // VCO: integrate frequency into phase
            state.frequency = freq;
            state.phase = state.phase + freq;

            freq
        })
    }

    pub fn params(&self) -> &P {
        &self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pll_default_state() {
        let state = PLLState::<f32>::default();
        assert_eq!(state.phase, 0.0);
        assert_eq!(state.frequency, 0.0);
        assert_eq!(state.pi.integral, 0.0);
        assert_eq!(state.pi.last_error, 0.0);
    }

    #[test]
    fn test_pll_phase_advances_by_frequency() {
        let params = PLLParamsPlain::new(1.0f32, 0.0f32, -100.0f32, 100.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        // With ki=0, output = kp * error = 1.0 * 2.0 = 2.0
        pll.update(2.0, &mut state);
        assert_eq!(state.frequency, 2.0);
        assert_eq!(state.phase, 2.0); // phase += frequency
    }

    #[test]
    fn test_pll_phase_accumulates() {
        let params = PLLParamsPlain::new(1.0f32, 0.0f32, -100.0f32, 100.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        pll.update(3.0, &mut state); // freq=3.0, phase=3.0
        pll.update(3.0, &mut state); // freq=3.0, phase=6.0
        pll.update(3.0, &mut state); // freq=3.0, phase=9.0

        assert_eq!(state.phase, 9.0);
    }

    #[test]
    fn test_pll_integral_builds_frequency() {
        // With kp=0, ki=1: frequency ramps up by error each step
        let params = PLLParamsPlain::new(0.0f32, 1.0f32, -1000.0f32, 1000.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        let f1 = pll.update(1.0, &mut state); // integral=1.0, freq=1.0
        let f2 = pll.update(1.0, &mut state); // integral=2.0, freq=2.0
        let f3 = pll.update(1.0, &mut state); // integral=3.0, freq=3.0

        assert_eq!(f1, 1.0);
        assert_eq!(f2, 2.0);
        assert_eq!(f3, 3.0);
    }

    #[test]
    fn test_pll_frequency_clamped_at_max() {
        let params = PLLParamsPlain::new(10.0f32, 0.0f32, -5.0f32, 5.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        let freq = pll.update(2.0, &mut state); // pterm = 20.0, clamped to 5.0
        assert_eq!(freq, 5.0);
        assert_eq!(state.frequency, 5.0);
    }

    #[test]
    fn test_pll_frequency_clamped_at_min() {
        let params = PLLParamsPlain::new(10.0f32, 0.0f32, -5.0f32, 5.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        let freq = pll.update(-2.0, &mut state); // pterm = -20.0, clamped to -5.0
        assert_eq!(freq, -5.0);
    }

    #[test]
    fn test_pll_antiwindup_upper() {
        // With large sustained positive error, integral should not grow unboundedly
        let params = PLLParamsPlain::new(1.0f32, 1.0f32, -10.0f32, 10.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        for _ in 0..20 {
            pll.update(100.0, &mut state);
        }

        // Frequency should be clamped at max
        assert_eq!(state.frequency, 10.0);
        // Integral should not have wound up — it was suppressed during saturation
        assert!(
            state.pi.integral < 20.0,
            "integral wound up: {}",
            state.pi.integral
        );
    }

    #[test]
    fn test_pll_antiwindup_lower() {
        let params = PLLParamsPlain::new(1.0f32, 1.0f32, -10.0f32, 10.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        for _ in 0..20 {
            pll.update(-100.0, &mut state);
        }

        assert_eq!(state.frequency, -10.0);
        assert!(
            state.pi.integral > -20.0,
            "integral wound up: {}",
            state.pi.integral
        );
    }

    #[test]
    fn test_pll_recovers_from_saturation() {
        let params = PLLParamsPlain::new(1.0f32, 0.5f32, -5.0f32, 5.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        // Saturate upward
        for _ in 0..10 {
            pll.update(100.0, &mut state);
        }
        assert_eq!(state.frequency, 5.0);

        // Now apply negative error — should recover quickly
        let freq = pll.update(-10.0, &mut state);
        assert!(freq < 5.0, "should have come off upper clamp, got {}", freq);
    }

    #[test]
    fn test_pll_zero_error_holds_state() {
        let params = PLLParamsPlain::new(1.0f32, 0.5f32, -100.0f32, 100.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        // Build up some integral
        pll.update(2.0, &mut state);
        pll.update(2.0, &mut state);
        let integral_before = state.pi.integral;

        // Zero error — integral should not change
        pll.update(0.0, &mut state);
        assert_eq!(state.pi.integral, integral_before);
    }

    #[test]
    fn test_pll_last_error_recorded() {
        let params = PLLParamsPlain::new(1.0f32, 0.0f32, -100.0f32, 100.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        pll.update(7.5, &mut state);
        assert_eq!(state.pi.last_error, 7.5);

        pll.update(-3.0, &mut state);
        assert_eq!(state.pi.last_error, -3.0);
    }

    #[test]
    fn test_pll_atomic_params() {
        let params = PLLParamsAtomic::new(2.0f32, 0.5f32, -50.0f32, 50.0f32);
        let pll = PLL::new(params);
        let mut state = PLLState::default();

        // P=2.0*1.0=2.0, I=0.5*1.0=0.5, total=2.5
        let freq = pll.update(1.0, &mut state);
        assert_eq!(freq, 2.5);

        // Tune kp live
        pll.params().set_kp(3.0);
        pll.params().with_snapshot(|p| assert_eq!(p.kp, 3.0));
    }
}
