//! Sector/phase geometry — the textbook 6-step convention this bench
//! standardized on (which differs from the Vimdrones silkscreen by an
//! A↔C swap; see minz/CLAUDE.md "Phase mapping").
//!
//! Purged to the single item the AM32-clone path reaches
//! (`minz::comp2::am32_change_comp_input` → [`edges_for`]).

/// Which COMP2 EXTI edges to enable for the given sector + edge-mode.
/// COMP2 polarity is locked to non-inverted (POLARITY=0); modes 3/4
/// (phys_ZC / phys_anti) split on sector parity so EXTI fires on the
/// expected physical V+/V− crossing direction for each sector.
///
///   0 (both): rising + falling — every COMP_VALUE transition
///   1 (raw_rise): rising only
///   2 (raw_fall): falling only
///   3 (phys_ZC): rising in even sectors (falling-BEMF ZC),
///                falling in odd sectors (rising-BEMF ZC)
///   4 (phys_anti): the opposite of mode 3 (diagnostic)
///   5 (val_gated): both edges at EXTI; the COMP ISR filters on VALUE
#[inline]
pub fn edges_for(mode: u8, sector: u8) -> (bool, bool) {
    match mode {
        0 => (true, true),
        1 => (true, false),
        2 => (false, true),
        3 => {
            if (sector & 1) == 0 {
                (true, false)
            } else {
                (false, true)
            }
        }
        4 => {
            if (sector & 1) == 0 {
                (false, true)
            } else {
                (true, false)
            }
        }
        _ => (true, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phys_zc_edge_polarity_by_sector_parity() {
        // Even sectors: falling BEMF → COMP rising edge expected.
        assert_eq!(edges_for(3, 0), (true, false));
        assert_eq!(edges_for(3, 1), (false, true));
        assert_eq!(edges_for(3, 4), (true, false));
        // Anti mode is the exact inverse.
        for sec in 0u8..6 {
            let (r, f) = edges_for(3, sec);
            assert_eq!(edges_for(4, sec), (f, r));
        }
        // Raw modes ignore sector.
        assert_eq!(edges_for(0, 3), (true, true));
        assert_eq!(edges_for(1, 3), (true, false));
        assert_eq!(edges_for(2, 3), (false, true));
        assert_eq!(edges_for(5, 3), (true, true));
    }
}
