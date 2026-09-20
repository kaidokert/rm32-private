//! Decoupled segment duty. Timed startup may hand off at the live 30% ceiling.
pub const MAX: u32 = 100;
pub const STARTUP_MAX: u32 = 300;
pub const fn segment_max(timed_startup: bool) -> u32 {
    if timed_startup { STARTUP_MAX } else { MAX }
}
pub fn select(acquisition: u32, requested: u32) -> Option<u32> {
    select_with_limit(acquisition, requested, 62, MAX)
}
pub fn select_startup(acquisition: u32, requested: u32) -> Option<u32> {
    select_with_limit(acquisition, requested, 100, STARTUP_MAX)
}
fn select_with_limit(
    acquisition: u32,
    requested: u32,
    limit: u32,
    result_limit: u32,
) -> Option<u32> {
    if !(40..=limit).contains(&acquisition) {
        return None;
    }
    let result = if requested == 0 {
        acquisition
    } else {
        requested
    };
    if (40..=result_limit).contains(&result) {
        Some(result)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_segment_ceiling_matches_initial_request_not_live_preload_state() {
        const TIMED: u32 = segment_max(true);
        const LEGACY: u32 = segment_max(false);
        assert_eq!((TIMED, LEGACY), (300, 100));
        for duty in 40..=301 {
            assert_eq!(select_startup(62, duty).is_some(), duty <= TIMED);
            assert_eq!(select(62, duty).is_some(), duty <= LEGACY);
        }
    }
    #[test]
    fn timed_startup_split_matches_drive_admission() {
        assert_eq!(select_startup(80, 70), Some(70));
        assert_eq!(select_startup(100, 0), Some(100));
        assert_eq!(select_startup(80, 200), Some(200));
        assert_eq!(select_startup(100, 300), Some(300));
        for (a, b) in [(39, 70), (101, 70), (80, 39), (80, 301)] {
            assert_eq!(select_startup(a, b), None);
        }
    }
    #[test]
    fn independent_without_raising_acquisition_limit() {
        assert_eq!(select(62, 0), Some(62));
        assert_eq!(select(62, 70), Some(70));
        assert_eq!(select(62, 100), Some(100));
        for (a, b) in [(63, 70), (39, 70), (62, 39), (62, 101), (62, 300)] {
            assert_eq!(select(a, b), None);
        }
    }
}
