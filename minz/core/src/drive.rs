//! Open-loop drive arithmetic and the sector/phase geometry tables —
//! the textbook 6-step convention this bench standardized on (which
//! differs from the Vimdrones silkscreen by an A↔C swap; see
//! minz/CLAUDE.md "Phase mapping").
//!
//! Phases are indexed 0=A, 1=B, 2=C throughout (matching
//! `comp2::ObservedPhase as u8`).

/// TIM7 drive-tick rate: the open-loop stepper and slew timebase.
pub const MOTOR_DRIVE_HZ: u32 = 6_000;

/// Compute the per-tick angle increment for a given electrical f.
/// Done in u64 to avoid overflow at high f. Returns 16.16 fixed
/// point degrees.
pub const fn angle_inc_fp(electrical_hz: u32) -> u32 {
    ((360u64 << 16) * electrical_hz as u64 / MOTOR_DRIVE_HZ as u64) as u32
}

/// Half-sector duration in µs at a given COMMANDED electrical
/// frequency — the open-loop ZC gate (closed loop uses the measured
/// interval via [`crate::timing::gate_us`] instead). 6 sectors per
/// electrical rev → full sector = `1e6 / (6·f)` µs; half is
/// `1e6 / (12·f)`.
pub const fn open_loop_gate_us(electrical_hz: u32) -> u32 {
    1_000_000 / (12 * electrical_hz)
}

/// Bitfield of float-window sectors for a given observed phase
/// (0=A, 1=B, 2=C). Textbook convention: A floats at sectors 2/5,
/// B at 1/4, C at 0/3.
pub const fn float_sector_mask(phase_idx: u8) -> u8 {
    match phase_idx {
        0 => (1 << 2) | (1 << 5),
        1 => (1 << 1) | (1 << 4),
        _ => (1 << 0) | (1 << 3),
    }
}

/// The two float sectors of a phase as a pair (used by the aligned
/// PWM-sample dump to place ZC midpoint markers).
pub const fn float_sectors(phase_idx: u8) -> (u8, u8) {
    match phase_idx {
        0 => (2, 5),
        1 => (1, 4),
        _ => (0, 3),
    }
}

/// Which phase floats in each sector (index by sector 0..=5).
/// Sector 0/3 → C, 1/4 → B, 2/5 → A — the CHAMELEON auto-mux table.
pub const SECTOR_FLOAT_PHASE_IDX: [u8; 6] = [2, 1, 0, 2, 1, 0];

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

/// Black-box commutation classes (indices into
/// [`crate::blackbox::EV_NAMES`]).
pub const BB_REF: u8 = 0;
pub const BB_BLD: u8 = 1;
pub const BB_DRK: u8 = 2;

/// Classify a closed-loop commutation by the window it ends:
/// dead-reckoned C windows (sectors 0/3) are DRK; A/B windows are
/// REF when an accepted ZC re-timed the shot, BLD when the blind
/// free-run fired.
pub fn commutation_class(prev_sector: u8, shot_refined: bool) -> u8 {
    if prev_sector == 0 || prev_sector == 3 {
        BB_DRK
    } else if shot_refined {
        BB_REF
    } else {
        BB_BLD
    }
}

pub const fn next_sector(prev: u8) -> u8 {
    (prev + 1) % 6
}

/// AM32 free-run semantics: every commutation immediately schedules
/// the next one at exactly **1.0×** the estimator interval; an
/// accepted ZC merely RE-TIMES the pending shot. (A 1.5×T "fallback"
/// compounded lag during acceleration until the watchdog fired —
/// that regression is why this trivial function exists as a named
/// contract.) `None` when the estimator is unseeded.
pub const fn freerun_reschedule_us(interval_us: u32) -> Option<u32> {
    if interval_us == 0 {
        None
    } else {
        Some(interval_us)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commutation_class_table() {
        // C windows are DRK regardless of refinement.
        assert_eq!(commutation_class(0, true), BB_DRK);
        assert_eq!(commutation_class(3, false), BB_DRK);
        // A/B windows split on whether the ZC re-timed the shot.
        for sec in [1u8, 2, 4, 5] {
            assert_eq!(commutation_class(sec, true), BB_REF);
            assert_eq!(commutation_class(sec, false), BB_BLD);
        }
    }

    #[test]
    fn sector_advance_wraps() {
        assert_eq!(next_sector(5), 0);
        assert_eq!(next_sector(0), 1);
    }

    #[test]
    fn regression_freerun_is_exactly_one_interval() {
        // 1.0×T, NOT 1.5×T — the compounded-lag desync incident.
        assert_eq!(freerun_reschedule_us(600), Some(600));
        assert_eq!(freerun_reschedule_us(0), None);
    }

    #[test]
    fn angle_inc_exact_at_reference_points() {
        // 6 kHz drive: f=60 Hz → 3.6°/tick = 235929.6 → truncates.
        assert_eq!(angle_inc_fp(60), (360u64 << 16) as u32 * 60 / 6000);
        // A full second of ticks must advance ~f revolutions.
        let inc = angle_inc_fp(100) as u64;
        let revs = inc * MOTOR_DRIVE_HZ as u64 / (360u64 << 16);
        assert_eq!(revs, 100);
    }

    #[test]
    fn open_loop_gate_is_half_sector() {
        assert_eq!(open_loop_gate_us(60), 1388); // 1.39 ms
        assert_eq!(open_loop_gate_us(600), 138);
    }

    #[test]
    fn float_geometry_consistent() {
        // mask ↔ pair ↔ per-sector table must all agree.
        for phase in 0u8..3 {
            let (s0, s1) = float_sectors(phase);
            assert_eq!(float_sector_mask(phase), (1 << s0) | (1 << s1));
            assert_eq!(SECTOR_FLOAT_PHASE_IDX[s0 as usize], phase);
            assert_eq!(SECTOR_FLOAT_PHASE_IDX[s1 as usize], phase);
        }
    }

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
