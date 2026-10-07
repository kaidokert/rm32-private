//! Exact constant-divisor arithmetic for Cortex-M0 motor-control paths.
//!
//! Thumb-v6M has no integer divide instruction. LLVM can therefore turn even
//! literal divisions into variable-latency `__aeabi_*div` calls.  These
//! helpers keep the bounded control-path cases in 32-bit multiply/shift/subtract
//! instructions. Their emitted use is enforced by the firmware link audit.

/// Exact truncating signed division by three, without a divide helper.
#[inline(always)]
pub const fn div3_i32(n: i32) -> i32 {
    if n < 0 {
        -(div3_u32(n.unsigned_abs()) as i32)
    } else {
        div3_u32(n as u32) as i32
    }
}

/// Exact unsigned division by three (Hacker's Delight shift/add form).
#[inline(always)]
pub const fn div3_u32(n: u32) -> u32 {
    let mut q = (n >> 2) + (n >> 4);
    q += q >> 4;
    q += q >> 8;
    q += q >> 16;
    let r = n - q * 3;
    q + ((11 * r) >> 5)
}

/// Exact unsigned division by five (shift/add estimate, then a bounded
/// remainder correction; no divide helper on M0).
#[inline(always)]
pub const fn div5_u32(n: u32) -> u32 {
    // q ~= n * 0.2 from below: 0.75 * (1 + 2^-4)(1 + 2^-8)(1 + 2^-16) / 4.
    let mut q = (n >> 1) + (n >> 2);
    q += q >> 4;
    q += q >> 8;
    q += q >> 16;
    q >>= 2;
    let mut r = n - q * 5;
    // The estimate is low by at most 2 (5.1 M samples incl. the top of the
    // range); three straight-line corrections, no loop on the ISR path.
    if r >= 5 {
        q += 1;
        r -= 5;
    }
    if r >= 5 {
        q += 1;
        r -= 5;
    }
    if r >= 5 {
        q += 1;
        r -= 5;
    }
    debug_assert!(r < 5);
    q
}

/// Exact `n / 2000` for any `u32`.
///
/// Fast path for products up to 16,000,000 (duty 2000 x ARR 8000; the
/// default ARRs are 1999 / 2665 / 3332 / 7082): since 2000 = 16 * 125,
/// discard the low four bits, then a ceil reciprocal with two bounded
/// corrections keeps the multiply inside `u32` on M0. Larger products occur
/// only with a low configured `pwm_frequency` (e.g. L431 at 8-10 kHz gives
/// ARR 8028-10021): they divide the reduced value by 5 three times (floor
/// division composes exactly), still without a divide helper. It used to
/// `assert!`, i.e. panic in the control ISR.
#[inline(always)]
pub const fn div2000_pwm(n: u32) -> u32 {
    if n > 16_000_000 {
        return div5_u32(div5_u32(div5_u32(n >> 4)));
    }
    let reduced = n >> 4;
    let mut q = (reduced * 4195) >> 19;
    if q * 125 > reduced {
        q -= 1;
    }
    if q * 125 > reduced {
        q -= 1;
    }
    q
}

/// Exact division by 1400 for the voltage-ramp numerator (0..=12,600).
/// At most nine subtracts execute, and only on a ramp-update tick.
#[inline(always)]
pub const fn div1400_ramp(n: u32) -> u32 {
    assert!(n <= 12_600);
    let mut q = 0;
    let mut r = n;
    while r >= 1400 {
        r -= 1400;
        q += 1;
    }
    q
}

/// Exact `n / 1910` for the sine-start duty map (AM32
/// `map(input, 137, 2047, minimum + 40, 2000)`): input span 1910 times a
/// duty span of at most 2000.
#[inline(always)]
pub const fn div1910_sine(n: u32) -> u32 {
    assert!(n <= 1910 * 2000);
    // ceil(2^21 / 1910) = 1098; the estimate is high by at most one.
    let mut q = (n * 1098) >> 21;
    if q * 1910 > n {
        q -= 1;
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn div3_is_exact_across_motor_domain_and_signed_edges() {
        for n in 0..=3 * u16::MAX as u32 {
            assert_eq!(div3_u32(n), n / 3, "n={n}");
        }
        for n in [u32::MAX, u32::MAX - 1, 0x8000_0000, 1_000_000_000] {
            assert_eq!(div3_u32(n), n / 3, "n={n}");
        }
        for n in [i32::MIN, -196_605, -1, 0, 1, 196_605, i32::MAX] {
            assert_eq!(div3_i32(n), n / 3, "n={n}");
        }
    }

    #[test]
    fn div2000_is_exact_across_declared_domain() {
        for n in 0..=16_000_000 {
            assert_eq!(div2000_pwm(n), n / 2000, "n={n}");
        }
    }

    #[test]
    fn div2000_pwm_is_exact_and_total_above_the_fast_range() {
        // Low pwm_frequency on L431 (ARR 10021) and G431 at full duty and
        // with a strength-9 drag brake: these used to panic in the ISR.
        for arr in [8028u32, 8885, 10021, 14164] {
            for duty in [1600u32, 1800, 2000] {
                let n = duty * arr;
                assert_eq!(div2000_pwm(n), n / 2000, "duty={duty} arr={arr}");
            }
        }
        for n in [16_000_000u32, 16_000_001, 20_042_000, u32::MAX] {
            assert_eq!(div2000_pwm(n), n / 2000, "n={n}");
        }
        // Dense sweep of the fallback range plus a deterministic LCG sample.
        let mut n: u32 = 16_000_001;
        while n < u32::MAX - 997 {
            assert_eq!(div2000_pwm(n), n / 2000, "n={n}");
            n += 997;
        }
        let mut x: u32 = 0x1234_5678;
        for _ in 0..2_000_000 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            assert_eq!(div2000_pwm(x), x / 2000, "x={x}");
        }
    }

    #[test]
    fn div5_is_exact_on_edges_and_samples() {
        for n in (0u32..200_000).chain([u32::MAX, u32::MAX - 1, u32::MAX - 4, 1 << 31]) {
            assert_eq!(div5_u32(n), n / 5, "n={n}");
        }
        let mut x: u32 = 0xdead_beef;
        for _ in 0..2_000_000 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            assert_eq!(div5_u32(x), x / 5, "x={x}");
        }
    }

    #[test]
    fn div1910_is_exact_across_declared_domain() {
        for n in 0..=1910 * 2000 {
            assert_eq!(div1910_sine(n), n / 1910, "n={n}");
        }
    }

    #[test]
    fn div1400_is_exact_across_declared_domain() {
        for n in 0..=12_600 {
            assert_eq!(div1400_ramp(n), n / 1400, "n={n}");
        }
    }
}
