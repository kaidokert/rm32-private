//! The replay test (goal item 7): the 25% loop's accept/refuse sequence, as
//! the COMP root decided it on the bench, pinned against the production
//! decision policy.
//!
//! `captures/replay/e121-capture25b.txt` holds 1536 consecutive COMP
//! decisions from a locked 25% run on the diagnostic `edge-capture` image
//! (E121), and the estimator's state at the first of them. Each decision's
//! inputs are the ISR's own: the count since the last acceptance, the edge
//! polarity, the advance, and every live comparator read the persistence
//! filter took. The test rebuilds the estimator from that state and offers
//! each decision to `Production`'s detector (`<Production as Policies>::B`:
//! its estimator type and `FILTER`), serving the recorded reads back in
//! order. The outcome, the wait, the new estimate and the number of reads
//! the filter asked for must all be what the hardware recorded. Any change
//! to the decision -- the gate, the filter schedule, the blend, the clamp,
//! the wait -- fails it.

extern crate std;

use std::vec::Vec;

use crate::bemf::{Outcome, ZeroCross};
use crate::capture::{Decision, Kind};

use super::policy::Bemf;
use super::{Policies, Production};

const CAPTURE: &str = include_str!("../../captures/replay/e121-capture25b.txt");

fn kv(line: &str, key: &str) -> u32 {
    line.split_whitespace()
        .find_map(|t| t.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
        .unwrap_or_else(|| panic!("no {key} in {line}"))
        .parse()
        .unwrap()
}

fn parse() -> ([u32; 5], Vec<Decision>) {
    let snap = CAPTURE.lines().find(|l| l.starts_with("CAPSNAP")).expect("CAPSNAP");
    let state = [
        kv(snap, "avg"),
        kv(snap, "last"),
        kv(snap, "min"),
        kv(snap, "max"),
        kv(snap, "blank"),
    ];
    let decisions: Vec<Decision> = CAPTURE
        .lines()
        .filter_map(|l| l.strip_prefix("CAP "))
        .map(|rest| {
            let f: Vec<u32> = rest.split_whitespace().map(|x| x.parse().unwrap()).collect();
            Decision {
                count: f[0] as u16,
                reads: f[1] as u16,
                wait: f[2] as u16,
                average: f[3] as u16,
                kind: match f[4] {
                    0 => Kind::Accepted,
                    1 => Kind::TooEarly,
                    2 => Kind::Unstable,
                    _ => Kind::Rebase,
                },
                rising: f[5] != 0,
                advance: f[6] as u8,
            }
        })
        .collect();
    assert_eq!(
        decisions.len(),
        kv(snap, "len") as usize,
        "every recorded decision parsed"
    );
    (state, decisions)
}

/// The replay itself: offer every recorded decision to the production
/// detector and require the recorded outcome.
#[test]
fn production_policy_reproduces_the_captured_25_percent_sequence() {
    type B = <Production as Policies>::B;
    let (state, decisions) = parse();
    // The captured estimator is the one the production policy builds: its
    // floor and its gate (the ceiling is 1.5 x that run's seed).
    let built = B::estimator(state[3] * 2 / 3).state();
    assert_eq!((state[2], state[4]), (built[2], built[4]), "floor and blanking");
    let mut zc = ZeroCross::from_state(state);
    let mut tally = [0u32; 4];
    for (i, d) in decisions.iter().enumerate() {
        tally[d.kind as usize] += 1;
        let count = u32::from(d.count);
        if d.kind == Kind::Rebase {
            assert!(count > zc.bounds().1, "#{i}: a re-base needs a count beyond the band");
            continue;
        }
        assert!(count <= zc.bounds().1, "#{i}: an offer needs a count inside the band");
        let mut taken = 0u16;
        let outcome = zc.offer(count, d.rising, u32::from(d.advance), &B::FILTER, || {
            let bit = (d.reads >> taken) & 1 == 1;
            taken += 1;
            bit
        });
        assert_eq!(
            taken,
            d.reads_taken(),
            "#{i}: the filter asked for a different number of reads"
        );
        match (outcome, d.kind) {
            (
                Outcome::Accepted {
                    wait, average_interval, ..
                },
                Kind::Accepted,
            ) => {
                assert_eq!(wait, u32::from(d.wait), "#{i}: wait");
                assert_eq!(average_interval, u32::from(d.average), "#{i}: new estimate");
            }
            (Outcome::TooEarly, Kind::TooEarly) | (Outcome::Unstable, Kind::Unstable) => {}
            (got, want) => {
                panic!("#{i}: recorded {want:?}, the policy decides {got:?} (count {count})")
            }
        }
    }
    // The sequence is a locked 25% loop's: acceptances, and both refusal
    // classes, all present.
    assert!(tally[Kind::Accepted as usize] > 100, "{tally:?}");
    assert!(tally[Kind::TooEarly as usize] > 100, "{tally:?}");
    assert!(tally[Kind::Unstable as usize] > 100, "{tally:?}");
    // And the estimate stays at the 25% sector throughout (~144 µs).
    let (a, b) = (zc.average_interval(), state[0]);
    assert!((130..=160).contains(&a) && (130..=160).contains(&b), "{b} -> {a}");
}

/// The replay is not vacuous: a policy one step different (the blanking gate
/// widened by one sixty-fourth) no longer reproduces the capture.
#[test]
fn a_one_step_different_policy_does_not_reproduce_it() {
    type B = <Production as Policies>::B;
    let (state, decisions) = parse();
    assert_eq!(state[4], 32, "the capture ran the half-cycle gate");
    let mut zc = crate::bemf::ZeroCrossWith::<33>::from_state(state);
    let mismatch = decisions.iter().filter(|d| d.kind != Kind::Rebase).any(|d| {
        let mut taken = 0u16;
        let o = zc.offer(u32::from(d.count), d.rising, u32::from(d.advance), &B::FILTER, || {
            let bit = (d.reads >> taken) & 1 == 1;
            taken += 1;
            bit
        });
        !matches!(
            (o, d.kind),
            (Outcome::Accepted { .. }, Kind::Accepted)
                | (Outcome::TooEarly, Kind::TooEarly)
                | (Outcome::Unstable, Kind::Unstable)
        ) || taken != d.reads_taken()
    });
    assert!(mismatch, "a different gate must decide some recorded edge differently");
}
