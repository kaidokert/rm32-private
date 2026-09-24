//! Post-transfer duty ramp: from the handover duty to the rung's target.
//!
//! The qualified image reached its higher rungs with "a 1%-per-0.5s ramp"
//! (`binz/DUTY_50_CAMPAIGN.md`, the 10-50% rung runs), starting from the
//! post-transfer BEMF duty `bemfdu100`, 10.0%
//! (`binz/LOW_DUTY_REPLICATION.md`: "`bemfdu100` sets the post-transfer BEMF
//! duty override to 10.0%"). firmware50 stepped straight to its target at
//! transfer (E069), which is benign from 10% to 15% but a large current step
//! at 25% (E083).
//!
//! Pure and host-tested; the caller supplies the time since transfer. The
//! division runs in the foreground, never in an ISR root.

/// Duty at the transfer instant, tenths of a percent (`bemfdu100`).
pub const START_TENTHS: u16 = 100;
/// Ramp increment, tenths of a percent (1%).
pub const STEP_TENTHS: u16 = 10;
/// Time per increment, µs (0.5 s).
pub const STEP_US: u32 = 500_000;

/// The duty `since_us` after transfer, ramping from `START_TENTHS` toward
/// `target_tenths` and holding there. A target at or below the start is
/// applied at once; the ramp never overshoots and never moves down.
#[must_use]
pub const fn duty_at(target_tenths: u16, since_us: u32) -> u16 {
    if target_tenths <= START_TENTHS {
        return target_tenths;
    }
    let steps = since_us / STEP_US;
    let span = (target_tenths - START_TENTHS) as u32;
    let up = steps.saturating_mul(STEP_TENTHS as u32);
    if up >= span {
        target_tenths
    } else {
        START_TENTHS + up as u16
    }
}

/// How long the ramp takes to reach `target_tenths`, µs.
#[must_use]
pub const fn ramp_us(target_tenths: u16) -> u32 {
    if target_tenths <= START_TENTHS {
        return 0;
    }
    let span = (target_tenths - START_TENTHS) as u32;
    let steps = span.div_ceil(STEP_TENTHS as u32);
    steps * STEP_US
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_the_reference_bemf_duty() {
        assert_eq!(duty_at(250, 0), START_TENTHS);
        assert_eq!(START_TENTHS, 100);
    }

    #[test]
    fn climbs_one_percent_per_half_second() {
        assert_eq!(duty_at(250, 499_999), 100);
        assert_eq!(duty_at(250, 500_000), 110);
        assert_eq!(duty_at(250, 1_000_000), 120);
        assert_eq!(duty_at(250, 7_000_000), 240);
        assert_eq!(duty_at(250, 7_500_000), 250);
    }

    #[test]
    fn holds_at_target_and_never_overshoots() {
        for target in [150u16, 200, 250, 155] {
            let mut prev = 0;
            for t in (0..20_000_000u32).step_by(100_000) {
                let d = duty_at(target, t);
                assert!(d <= target, "{target}: {d} at {t}");
                assert!(d >= prev, "never moves down");
                prev = d;
            }
            assert_eq!(duty_at(target, u32::MAX), target);
        }
    }

    #[test]
    fn a_target_at_or_below_the_start_applies_at_once() {
        assert_eq!(duty_at(100, 0), 100);
        assert_eq!(duty_at(62, 0), 62);
    }

    #[test]
    fn ramp_durations_per_rung() {
        assert_eq!(ramp_us(150), 2_500_000);
        assert_eq!(ramp_us(200), 5_000_000);
        assert_eq!(ramp_us(250), 7_500_000);
        assert_eq!(ramp_us(100), 0);
        // The ramp reaches target exactly at its stated duration.
        for t in [150u16, 200, 250, 155] {
            assert_eq!(duty_at(t, ramp_us(t)), t, "{t}");
        }
    }
}
