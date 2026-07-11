//! Key-handler arithmetic: clamps, cyclers, and their display names.
//! Trivial individually, but these clamps ARE safety bounds (AMP_MAX
//! is the envelope ceiling; blank > 50 µs gates whole PWM cycles;
//! negative advance collapses the rotor) — so they get tests.

/// Clamp a throttle-percent adjustment into `[min, max]`.
pub fn clamp_pct(v: i32, min: u16, max: u16) -> u16 {
    v.clamp(min as i32, max as i32) as u16
}

/// Clamp a frequency adjustment into `[min, max]` Hz.
pub fn clamp_hz(v: i32, min: u32, max: u32) -> u32 {
    v.clamp(min as i32, max as i32) as u32
}

/// COMP2 hysteresis cycle: 0 → 1 → 3 → 0 (skip 2/medium — bench only
/// wants none / low / high).
pub fn next_hyst(cur: u8) -> u8 {
    match cur {
        0 => 1,
        1 => 3,
        _ => 0,
    }
}

pub fn hyst_name(level: u8) -> &'static str {
    match level {
        0 => "0/none",
        1 => "1/low",
        _ => "3/high",
    }
}

/// EXTI edge-mode cycle 0..=5 (see [`crate::drive::edges_for`]).
pub fn next_edge_mode(cur: u8) -> u8 {
    if cur >= 5 { 0 } else { cur + 1 }
}

pub fn edge_mode_name(mode: u8) -> &'static str {
    match mode {
        0 => "both",
        1 => "raw rise",
        2 => "raw fall",
        3 => "phys ZC (rise even / fall odd)",
        4 => "phys anti-ZC (fall even / rise odd)",
        _ => "value-gated (both edges, VALUE=1 only)",
    }
}

/// Short form for dump headers.
pub fn edge_mode_short(mode: u8) -> &'static str {
    match mode {
        0 => "both",
        1 => "raw_rise",
        2 => "raw_fall",
        3 => "phys_ZC",
        4 => "phys_anti",
        _ => "val_gated",
    }
}

/// Software COMP-blank adjustment for the `n`/`N`/`.`/`,` keys,
/// clamped to [0, 50] µs (one 24 kHz PWM cycle is 41.67 µs; above
/// 50 µs whole cycles — including legitimate BEMF edges — are gated).
pub fn blank_adjust(cur: u16, key: u8) -> u16 {
    let delta: i32 = match key {
        b'n' => 1,
        b'N' => -1,
        b'.' => 10,
        _ => -10,
    };
    (cur as i32 + delta).clamp(0, 50) as u16
}

/// Manual advance ±2°, clamped 0..=28. Negative advance is proven
/// destructive (collapses the rotor to a crawl the loop then tracks).
pub fn advance_adjust(cur: i8, up: bool) -> i8 {
    (cur + if up { 2 } else { -2 }).clamp(0, 28)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct_and_hz_clamps_hold_envelope() {
        assert_eq!(clamp_pct(-5, 0, 60), 0);
        assert_eq!(clamp_pct(61, 0, 60), 60);
        assert_eq!(clamp_pct(35, 0, 60), 35);
        assert_eq!(clamp_hz(0, 1, 600), 1);
        assert_eq!(clamp_hz(9999, 1, 600), 600);
    }

    #[test]
    fn hyst_cycle_skips_medium() {
        assert_eq!(next_hyst(0), 1);
        assert_eq!(next_hyst(1), 3);
        assert_eq!(next_hyst(3), 0);
    }

    #[test]
    fn edge_mode_wraps_at_five() {
        let mut m = 0;
        for _ in 0..6 {
            m = next_edge_mode(m);
        }
        assert_eq!(m, 0);
    }

    #[test]
    fn blank_clamps_zero_to_fifty() {
        assert_eq!(blank_adjust(0, b'N'), 0);
        assert_eq!(blank_adjust(0, b','), 0);
        assert_eq!(blank_adjust(45, b'.'), 50);
        assert_eq!(blank_adjust(8, b'n'), 9);
        assert_eq!(blank_adjust(8, b','), 0);
    }

    #[test]
    fn names_cover_every_mode() {
        assert_eq!(hyst_name(0), "0/none");
        assert_eq!(hyst_name(1), "1/low");
        assert_eq!(hyst_name(3), "3/high");
        for m in 0u8..6 {
            assert!(!edge_mode_name(m).is_empty());
            assert!(!edge_mode_short(m).contains(' '), "short names are tokens");
        }
        assert_eq!(edge_mode_short(3), "phys_ZC");
        assert_eq!(edge_mode_name(0), "both");
        assert_eq!(edge_mode_short(1), "raw_rise");
        assert_eq!(edge_mode_short(2), "raw_fall");
        assert_eq!(edge_mode_short(4), "phys_anti");
        assert_eq!(edge_mode_short(5), "val_gated");
        assert_eq!(edge_mode_name(1), "raw rise");
        assert_eq!(edge_mode_name(2), "raw fall");
        assert!(edge_mode_name(4).starts_with("phys anti"));
        assert!(edge_mode_name(5).starts_with("value-gated"));
        assert!(edge_mode_name(3).starts_with("phys ZC"));
    }

    #[test]
    fn advance_never_negative_never_past_28() {
        assert_eq!(advance_adjust(0, false), 0);
        assert_eq!(advance_adjust(28, true), 28);
        assert_eq!(advance_adjust(4, true), 6);
        assert_eq!(advance_adjust(4, false), 2);
    }
}
