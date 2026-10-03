//! Reverse-only high-duty advance A/B. Levels are minz-core's 0..64 scale.
pub const fn level(duty_tenths: u32) -> u16 {
    if duty_tenths >= 350 { 22 } else { 20 }
}

#[cfg(test)]
mod tests {
    use super::level;

    #[test]
    fn boundary_is_exact_and_bounded() {
        for duty in [0, 100, 250, 349] {
            assert_eq!(level(duty), 20);
        }
        for duty in [350, 351, 450, 500] {
            assert_eq!(level(duty), 22);
        }
    }
}
