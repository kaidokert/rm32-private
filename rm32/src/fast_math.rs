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

/// Exact `n / 2000` for the complete rm32 PWM-product domain.
///
/// All supported timers have ARR <= 7082, so a duty-scale product is at most
/// 14,164,000. The 16,000,000 contract leaves explicit headroom while keeping
/// the reciprocal multiply inside `u32` on M0. Since 2000 = 16 * 125, discard
/// the low four bits first, then use a ceil reciprocal with two bounded
/// corrections.
#[inline(always)]
pub const fn div2000_pwm(n: u32) -> u32 {
    assert!(n <= 16_000_000);
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
    fn div1400_is_exact_across_declared_domain() {
        for n in 0..=12_600 {
            assert_eq!(div1400_ramp(n), n / 1400, "n={n}");
        }
    }
}
