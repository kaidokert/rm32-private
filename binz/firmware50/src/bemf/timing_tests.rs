use super::*;

struct TestDepth(u8);
impl FilterPolicy for TestDepth {
    fn level(&self, _: u32) -> u8 { self.0 }
}

fn predicate_sequences<T: WaitEstimate>() {
    for depth in 0..=12u8 {
        for bits in 0..(1u32 << depth) {
            for rising in [false, true] {
                for count in [0, 40, 41, 160] {
                    let mut old = ZeroCrossWith::<32>::new_bounded(80, 40, 4000);
                    let mut new = old.clone();
                    let (mut oi, mut ni) = (0u32, 0u32);
                    let a = old.historical_offer_timed::<T, _, _>(count, rising, 16, &TestDepth(depth), || {
                        let v = bits & (1 << oi) != 0;
                        oi += 1;
                        v
                    });
                    let expected = u32::from(rising) << 30;
                    let b = new.offer_timed_matches::<T, _, _>(count, 16, &TestDepth(depth), || {
                        let word = (((bits >> ni) & 1) << 30) | !(1 << 30);
                        ni += 1;
                        word & (1 << 30) == expected
                    });
                    let first_bad = (0..u32::from(depth))
                        .find(|i| (bits & (1 << i) != 0) != rising);
                    let want_reads = if count <= 40 { 0 }
                        else { first_bad.map_or(u32::from(depth), |i| i + 1) };
                    assert_eq!((oi, ni), (want_reads, want_reads));
                    assert_eq!(a, b);
                    assert_eq!(old.state(), new.state());
                    assert_eq!(old.counts(), new.counts());
                    assert_eq!(matches!(b, Outcome::Accepted { .. }), count > 40 && first_bad.is_none());
                }
            }
        }
    }
}

#[test]
fn matched_predicate_preserves_every_stream_through_twelve_reads() {
    predicate_sequences::<FreshEstimate>();
    predicate_sequences::<PreviousEstimate>();
}

fn historical_sequences<T: WaitEstimate>() {
    let mut old = ZeroCross::new_bounded(80, 40, 4000);
    old.accepted = u32::MAX - 2;
    old.too_early = u32::MAX - 2;
    old.unstable = u32::MAX - 2;
    let mut new = old.clone();
    let mut rng = 7u32;
    for i in 0..20_000 {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let count = match i % 5 { 0 => 0, 1 => u32::MAX, _ => rng & 8191 };
        let depth = [0, 1, 5, 12, 255][i % 5];
        let rising = i % 2 == 0;
        let advance = [0, 16, 64, u32::MAX][i % 4];
        let (mut oi, mut ni) = (0u32, 0u32);
        let level = |j| i % 3 == 0 || rng.rotate_right(j) & 1 != 0;
        let a = old.historical_offer_timed::<T, _, _>(
            count, rising, advance, &TestDepth(depth), || {
                let v = level(oi); oi += 1; v
            });
        let b = new.offer_timed_matches::<T, _, _>(
            count, advance, &TestDepth(depth), || {
                let v = level(ni); ni += 1; v == rising
            });
        assert_eq!(a, b);
        assert_eq!(oi, ni);
        assert_eq!(old.state(), new.state());
        assert_eq!(old.prev_zc, new.prev_zc);
        assert_eq!(old.counts(), new.counts());
    }
    assert!(old.accepted < 20_000 && old.unstable < 20_000 && old.too_early < 20_000);
}

#[test]
fn historical_detector_matches_stateful_sequences_and_counter_wrap() {
    historical_sequences::<FreshEstimate>();
    historical_sequences::<PreviousEstimate>();
    historical_sequences::<ConstantAdvance<FreshEstimate, 16>>();
    historical_sequences::<ConstantAdvance<PreviousEstimate, 16>>();
}

fn accepted_wait(o: Outcome) -> u32 {
    match o {
        Outcome::Accepted { wait, .. } => wait,
        other => panic!("expected acceptance, got {other:?}"),
    }
}

fn fixed16_equivalence<T: WaitEstimate>() {
    for seed in 1..=65535 {
        let mut dynamic = ZeroCross::new_bounded(seed, 1, u32::MAX);
        let mut fixed = dynamic.clone();
        let a = dynamic.offer_timed::<T, _, _>(seed * 2, true, 16, &FixedFilter::<5>, || true);
        let b = fixed.offer_timed::<ConstantAdvance<T, 16>, _, _>(
            seed * 2, true, 16, &FixedFilter::<5>, || true);
        assert_eq!(a, b);
        assert_eq!(dynamic.state(), fixed.state());
        assert_eq!(dynamic.counts(), fixed.counts());
        for count in [0, 1, 40, 65535, u32::MAX] {
            for level in [false, true] {
                let a = dynamic.offer_timed::<T, _, _>(count, true, 16, &FixedFilter::<5>, || level);
                let b = fixed.offer_timed::<ConstantAdvance<T, 16>, _, _>(
                    count, true, 16, &FixedFilter::<5>, || level);
                assert_eq!(a, b);
                assert_eq!(dynamic.state(), fixed.state());
                assert_eq!(dynamic.counts(), fixed.counts());
            }
        }
    }
}

#[test]
fn constant16_preserves_fresh_and_previous_across_full_timer_range() {
    fixed16_equivalence::<FreshEstimate>();
    fixed16_equivalence::<PreviousEstimate>();
}

#[test]
fn previous_wait_uses_seed_then_preblend_state_and_exact_odd_rounding() {
    for ci in 1..4096 {
        for level in [0, 16, 18, 32, 64, u32::MAX] {
            let mut z = ZeroCross::new_bounded(ci, 1, 8192);
            let o = z.offer_timed::<PreviousEstimate, _, _>(ci * 2, true, level, &FixedFilter::<1>, || true);
            assert_eq!(accepted_wait(o), wait_time(ci, level));
        }
    }
    assert_eq!(wait_time(59, 16), 15); // not truncating 59/4 = 14
}

#[test]
fn timing_selection_preserves_all_state_and_refusal_decisions() {
    let mut fresh = ZeroCross::new_bounded(65, 40, 4000);
    let mut prior = fresh.clone();
    let mut state = 7u32;
    for i in 0..20_000 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let count = state & 8191;
        let level = i % 3 != 0;
        let before = prior.average_interval();
        let a = fresh.offer(count, true, 16, &FixedFilter::<5>, || level);
        let b = prior.offer_timed::<PreviousEstimate, _, _>(count, true, 16, &FixedFilter::<5>, || level);
        assert_eq!(fresh.state(), prior.state());
        assert_eq!(fresh.counts(), prior.counts());
        assert_eq!(fresh.prev_zc, prior.prev_zc);
        assert_eq!(fresh.blanking(), prior.blanking());
        match (a, b) {
            (Outcome::Accepted { .. }, Outcome::Accepted { .. }) => {
                assert_eq!(accepted_wait(a), wait_time(fresh.average_interval(), 16));
                assert_eq!(accepted_wait(b), wait_time(before, 16));
            }
            _ => assert_eq!(a, b),
        }
    }
}

#[test]
fn frozen_e383_suffix_replays_fresh_waits_but_prior_dependency_changes_them() {
    // E383 ORDERA228826..228833: (interval, new average, recorded wait).
    // First row seeds the second's exact pre-state: previous interval38, avg59.
    let rows = [(67, 55, 14), (52, 57, 14), (60, 56, 14), (64, 59, 15),
        (76, 64, 16), (34, 59, 15), (57, 52, 13)];
    let mut fresh = ZeroCross::from_state([59, 38, 40, 4000, 32]);
    let mut prior = fresh.clone();
    for (count, average, wait) in rows {
        let before = prior.average_interval();
        let a = fresh.offer(count, true, 16, &FixedFilter::<1>, || true);
        let b = prior.offer_timed::<PreviousEstimate, _, _>(count, true, 16, &FixedFilter::<1>, || true);
        assert_eq!(accepted_wait(a), wait);
        assert_eq!(accepted_wait(b), wait_time(before, 16));
        assert_eq!(fresh.average_interval(), average);
        assert_eq!(fresh.state(), prior.state());
    }
    // Inputs stay frozen; this is NOT a prediction of the changed rotor path.
}

#[test]
fn extreme_widths_clamps_and_refusals_preserve_contract() {
    for seed in [0, 1, 39, 40, 4000, 65535, u32::MAX] {
        let mut z = ZeroCross::new_bounded(seed, 40, 4000);
        let before = z.state();
        assert_eq!(z.offer_timed::<PreviousEstimate, _, _>(0, true, 16, &FixedFilter::<1>, || true), Outcome::TooEarly);
        assert_eq!(z.state(), before);
        let o = z.offer_timed::<PreviousEstimate, _, _>(u32::MAX, true, 16, &FixedFilter::<1>, || true);
        assert_eq!(accepted_wait(o), wait_time(seed, 16));
        assert!((40..=4000).contains(&z.average_interval()));
    }
}

#[test]
fn e425_contraction_and_rebound_expose_both_wait_policy_tradeoffs() {
    // Raw ORDERA420322 seeds420323..420340 (E425 diode-order capture).
    // Tuples: service interval, published average, recorded prior wait (us).
    let rows = [(84, 74, 19), (64, 74, 19), (61, 68, 19), (75, 68, 17),
        (60, 67, 17), (58, 63, 17), (55, 59, 16), (60, 58, 15),
        (60, 59, 15), (47, 56, 15), (52, 52, 14), (46, 50, 13),
        (49, 48, 13), (49, 48, 12), (35, 45, 12), (28, 40, 11),
        (49, 40, 10), (63, 48, 10)];
    let mut prior = ZeroCross::from_state([76, 62, 40, 4000, 32]);
    let mut fresh = prior.clone();
    let mut shorter = 0;
    let mut last = (0, 0);
    for (interval, average, recorded) in rows {
        let p = accepted_wait(prior.offer_timed::<PreviousEstimate, _, _>(
            interval, true, 16, &FixedFilter::<1>, || true));
        let f = accepted_wait(fresh.offer_timed::<FreshEstimate, _, _>(
            interval, true, 16, &FixedFilter::<1>, || true));
        assert_eq!(p, recorded);
        assert_eq!(prior.average_interval(), average);
        assert_eq!(prior.state(), fresh.state());
        shorter += u32::from(f < p);
        last = (p, f);
    }
    assert_eq!(last, (10, 12), "fresh helps the final rebound");
    assert!(shorter > 0, "fresh also tightens earlier arms on descent");
    // Frozen accepted inputs omit rejections and physical rotor feedback.
    // This cannot predict survival or qualify the alternative on hardware.
}
