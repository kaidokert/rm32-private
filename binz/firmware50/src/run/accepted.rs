//! Preserve delivered event deltas from a latest-value ISR mailbox (E362).
//!
//! Intermediate timestamps may coalesce, but event counts must not. Read the
//! published sequence around the stamp; if interrupted by a publication, defer
//! to the next foreground pass without consuming anything. There is no retry
//! loop, hardware fence, interrupt mask, or added ISR work.
//! Startup/hold boundaries and pending events at stop are not reconstructed.

use core::num::NonZeroU32;
use core::sync::atomic::{compiler_fence, Ordering};

/// One coherent latest timestamp and the number of events since last delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accepted {
    pub raw: u16,
    pub count: NonZeroU32,
}

impl Accepted {
    /// Ordinary non-coalesced delivery (also used by the host simulator).
    #[must_use]
    pub const fn single(raw: u16) -> Self {
        Self {
            raw,
            count: NonZeroU32::MIN,
        }
    }
}

/// Foreground only, on the single-core MCU: the ISR publishes stamp before
/// sequence and completes before foreground resumes. `seen` advances only for
/// a coherent pair. Wrapping subtraction supports a u32 sequence wrap; the
/// watchdog excludes going an entire sequence modulus without servicing it.
#[inline(always)]
pub fn observe(seen: &mut u32, mut sequence: impl FnMut() -> u32, stamp: impl FnOnce() -> u16) -> Option<Accepted> {
    let before = sequence();
    if before == *seen {
        return None;
    }
    compiler_fence(Ordering::SeqCst);
    let raw = stamp();
    compiler_fence(Ordering::SeqCst);
    if sequence() != before {
        return None;
    }
    let count = NonZeroU32::new(before.wrapping_sub(*seen))?;
    *seen = before;
    Some(Accepted { raw, count })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accept(raw: u16, count: u32) -> Option<Accepted> {
        Some(Accepted {
            raw,
            count: NonZeroU32::new(count).unwrap(),
        })
    }

    #[test]
    fn coalesced_events_keep_their_count_and_latest_stamp() {
        let mut seen = 10;
        assert_eq!(observe(&mut seen, || 14, || 90), accept(90, 4));
        assert_eq!(seen, 14);
        assert_eq!(observe(&mut seen, || 14, || panic!("unchanged")), None);
        assert_eq!(observe(&mut seen, || 15, || 150), accept(150, 1));
    }

    #[test]
    fn interrupted_pair_is_deferred_without_losing_events() {
        let mut seen = 10;
        let mut sequence = [14, 15].into_iter();
        assert_eq!(observe(&mut seen, || sequence.next().unwrap(), || 100), None);
        assert_eq!(seen, 10);
        assert_eq!(observe(&mut seen, || 15, || 160), accept(160, 5));
    }

    #[test]
    fn count_and_raw_stamp_wrap_independently() {
        let mut seen = u32::MAX - 1;
        assert_eq!(observe(&mut seen, || 1, || 2), accept(2, 3));
        assert_eq!(seen, 1);
    }
}
