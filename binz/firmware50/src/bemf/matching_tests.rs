use super::*;

/// Frozen pre-E467 decision, independent of both new entry points. Shared
/// arithmetic helpers were not changed by E467; fields/counters are compared.
fn old_offer<T: WaitEstimate>(
    z: &mut ZeroCross,
    count: u32,
    rising: bool,
    advance: u32,
    depth: u8,
    mut read: impl FnMut() -> bool,
) -> Outcome {
    if count <= z.blanking() {
        z.too_early = z.too_early.wrapping_add(1);
        return Outcome::TooEarly;
    }
    for _ in 0..depth {
        if read() != rising {
            z.unstable = z.unstable.wrapping_add(1);
            return Outcome::Unstable;
        }
    }
    let previous = z.average_interval;
    z.prev_zc = z.last_zc;
    z.last_zc = count;
    z.average_interval = z.clamp_interval(blend_interval(z.average_interval, z.prev_zc, z.last_zc));
    let scheduled = if T::PREVIOUS { previous } else { z.average_interval };
    let level = T::ADVANCE.unwrap_or(advance);
    z.accepted = z.accepted.wrapping_add(1);
    Outcome::Accepted {
        wait: wait_time(scheduled, level),
        average_interval: z.average_interval,
        advance: advance_of(scheduled, level),
    }
}

fn exhaustive_reads<T: WaitEstimate>() {
    for rising in [false, true] {
        for depth in 0..=255u8 {
            for dissent in 0..=u16::from(depth) {
                let mut boolean = ZeroCross::new_bounded(80, 40, 4000);
                let mut matching = boolean.clone();
                let (mut an, mut bn) = (0u16, 0u16);
                let a = old_offer::<T>(&mut boolean, 80, rising, 16, depth, || {
                    let level = if an == dissent { !rising } else { rising };
                    an += 1;
                    level
                });
                let b = matching.offer_matching_timed::<T, _, _>(80, 16, &DepthSnapshot(depth), || {
                    let agrees = bn != dissent;
                    bn += 1;
                    agrees
                });
                let all_match = dissent == u16::from(depth);
                let reads = if all_match { u16::from(depth) } else { dissent + 1 };
                assert_eq!((an, bn), (reads, reads));
                assert_eq!(matches!(a, Outcome::Accepted { .. }), all_match);
                assert_eq!(a, b);
                assert_eq!(boolean.state(), matching.state());
                assert_eq!(boolean.counts(), matching.counts());
                assert_eq!(boolean.prev_zc, matching.prev_zc);
            }
        }
    }
}

#[test]
fn matching_preserves_every_depth_polarity_and_dissent() {
    exhaustive_reads::<FreshEstimate>();
    exhaustive_reads::<PreviousEstimate>();
    exhaustive_reads::<ConstantAdvance<FreshEstimate, 16>>();
}

fn histories<T: WaitEstimate>() {
    for seed in [1, 40, 45, 59, 80, 65535, u32::MAX / 2, u32::MAX] {
        let mut a = ZeroCross::new_bounded(seed, 1, u32::MAX);
        let mut b = a.clone();
        for i in 0..128u32 {
            let blank = a.blanking();
            let count = [blank.saturating_sub(1), blank, blank.saturating_add(1), seed][(i & 3) as usize];
            let rising = i & 1 == 0;
            let advance = [0, 16, 22, 64, u32::MAX][i as usize % 5];
            let depth = DepthSnapshot((i % 13) as u8);
            let (mut an, mut bn) = (0u8, 0u8);
            let x = old_offer::<T>(&mut a, count, rising, advance, depth.0, || {
                an += 1;
                if u32::from(an) == i % 14 { !rising } else { rising }
            });
            let y = b.offer_matching_timed::<T, _, _>(count, advance, &depth, || {
                bn += 1;
                u32::from(bn) != i % 14
            });
            assert_eq!((x, an), (y, bn));
            assert_eq!(a.state(), b.state());
            assert_eq!(a.counts(), b.counts());
            assert_eq!(a.prev_zc, b.prev_zc);
            if count <= blank { assert_eq!((an, bn), (0, 0)); }
        }
    }
}

#[test]
fn matching_preserves_gates_extremes_and_history() {
    histories::<FreshEstimate>();
    histories::<PreviousEstimate>();
    histories::<ConstantAdvance<FreshEstimate, 16>>();
}

#[test]
fn encoded_value_comparison_ignores_other_register_bits() {
    for high in [false, true] {
        let expected = if high { 1u32 << 30 } else { 0 };
        for value in [0, 1 << 30, u32::MAX, !(1 << 30), 0x5555_5555, 0xaaaa_aaaa] {
            assert_eq!((value & (1 << 30)) == expected, ((value >> 30) & 1 != 0) == high);
        }
    }
}
