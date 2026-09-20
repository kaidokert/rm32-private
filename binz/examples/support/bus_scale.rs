//! Exact legacy floor(n*1194/100), with a bounded M0-friendly normal path.
//! Preserve release-mode wrapping multiplication outside the proven bound.
#[inline(always)]
pub fn scale(n: u32) -> u32 {
    const MAX: u32 = 4095;
    const SHIFT: u32 = 21;
    const SCALE: u32 = 1 << SHIFT;
    const RECIP: u32 = (SCALE + 99) / 100;
    const MAX_NUM: u32 = MAX * 6 + 99;
    const _: () = {
        assert!(RECIP == 20972);
        assert!(MAX_NUM <= u32::MAX / RECIP);
        assert!(MAX_NUM * (RECIP * 100 - SCALE) < SCALE);
    };
    // floor(11.94*n) = 12*n - ceil(0.06*n).
    if n <= MAX {
        n * 12 - ((n * 6 + 99) * RECIP >> SHIFT)
    } else {
        fallback(n)
    }
}
#[cold]
#[inline(never)]
fn fallback(n: u32) -> u32 {
    core::hint::black_box(n).wrapping_mul(1194) / 100
}

#[cfg(test)]
mod tests {
    #[test]
    fn exhaustive_normal_and_adjacent_domain() {
        for n in 0..=65535 {
            assert_eq!(super::scale(n), n.wrapping_mul(1194) / 100, "n={n}");
        }
    }
    #[test]
    fn overflow_and_wide_fallback() {
        for n in [
            u32::MAX,
            u32::MAX / 1194,
            u32::MAX / 1194 + 1,
            1_000_000,
            4096,
        ] {
            assert_eq!(super::scale(n), n.wrapping_mul(1194) / 100);
        }
    }
}
