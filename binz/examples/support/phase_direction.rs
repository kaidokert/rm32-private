//! Compile-time physical phase relabeling. A/B swap reverses the six-step
//! sequence while the controller and BEMF estimator retain logical A/B/C.

#[inline(always)]
pub const fn physical_phase(phase: u8) -> u8 {
    #[cfg(feature = "bench-reverse-phases")]
    {
        match phase {
            0 => 1,
            1 => 0,
            _ => phase,
        }
    }
    #[cfg(not(feature = "bench-reverse-phases"))]
    {
        phase
    }
}

#[inline(always)]
pub const fn physical_step(step: u8) -> u8 {
    #[cfg(feature = "bench-reverse-phases")]
    {
        match step {
            1 => 4,
            2 => 3,
            3 => 2,
            4 => 1,
            5 => 6,
            6 => 5,
            _ => step,
        }
    }
    #[cfg(not(feature = "bench-reverse-phases"))]
    {
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROLES: [(u8, u8, u8); 6] = [
        (0, 1, 2), (2, 1, 0), (2, 0, 1),
        (1, 0, 2), (1, 2, 0), (0, 2, 1),
    ];

    #[test]
    fn every_role_and_floating_sense_follow_the_same_permutation() {
        for step in 1..=6 {
            let old = ROLES[step - 1];
            let new = ROLES[physical_step(step as u8) as usize - 1];
            assert_eq!(new, (
                physical_phase(old.0), physical_phase(old.1), physical_phase(old.2)
            ));
        }
    }

    #[test]
    fn reverse_build_is_opposite_order() {
        #[cfg(feature = "bench-reverse-phases")]
        assert_eq!((1..=6).map(physical_step).collect::<Vec<_>>(), [4, 3, 2, 1, 6, 5]);
    }
}
