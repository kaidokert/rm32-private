//! One source of truth for closed-loop live-duty authority.
//! Forced startup and initial BEMF handoff retain their separate low-duty caps.
pub const MIN: u32 = 40;
pub const STEP: u32 = 50;
pub const MAX: u32 = if cfg!(feature = "bench-duty-50") {
    500
} else {
    300
};

pub const fn contains(duty: u32) -> bool {
    duty >= MIN && duty <= MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_explicit_and_bounded() {
        assert!(contains(MIN));
        assert!(contains(MAX));
        assert!(!contains(MIN - 1));
        assert!(!contains(MAX + 1));
        assert_eq!(STEP, 50);
        assert_eq!(
            MAX,
            if cfg!(feature = "bench-duty-50") {
                500
            } else {
                300
            }
        );
    }
}
