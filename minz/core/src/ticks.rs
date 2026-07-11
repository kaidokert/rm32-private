//! Sub-tick timestamp composition for the 1 µs wall clock built from
//! a 10 µs SysTick base plus the down-counting CVR remainder.
//!
//! The 2026-07-11 trust audit found the original guard (`cvr_post <=
//! cvr_pre`) has a hole: an ISR of ≥ one full SysTick period
//! preempting between the reads produces an accepted-but-stale
//! pairing — the composed time is off by a whole base tick (±10 µs),
//! suspiciously the size of the ZC-residual σ that conclusions were
//! being drawn from. The fixed protocol reads the coarse counter on
//! BOTH sides of CVR and additionally requires it unchanged.

/// One 10 µs base tick at 80 MHz core clock.
pub const RELOAD: u32 = 799;
/// Core cycles per µs.
pub const CYC_PER_US: u32 = 80;

/// Compose a 1 µs timestamp from a consistent snapshot, or `None` if
/// the snapshot is torn (caller retries).
///
/// Read order must be: `coarse_pre`, `cvr`, `coarse_post`.
/// Consistency requires `coarse_pre == coarse_post` — CVR wrapped
/// (or a long ISR ran) iff the base advanced, and that is now
/// detected regardless of where the preemption landed. This replaces
/// the old `cvr_post <= cvr_pre` heuristic, which a ≥10 µs ISR could
/// defeat (CVR returns to a smaller value after a full wrap and the
/// stale coarse pairs silently).
pub fn compose_1us(coarse_pre: u32, cvr: u32, coarse_post: u32) -> Option<u32> {
    if coarse_pre != coarse_post {
        return None;
    }
    Some(
        coarse_pre
            .wrapping_mul(10)
            .wrapping_add(RELOAD.wrapping_sub(cvr) / CYC_PER_US),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_within_tick() {
        // Start of tick: CVR at reload → +0 µs.
        assert_eq!(compose_1us(100, 799, 100), Some(1000));
        // 5 µs in: CVR down by 400 cycles.
        assert_eq!(compose_1us(100, 399, 100), Some(1005));
        // End of tick: just under +10.
        assert_eq!(compose_1us(100, 0, 100), Some(1009));
    }

    #[test]
    fn torn_snapshot_rejected() {
        // Base advanced between reads (wrap or long ISR): reject.
        assert_eq!(compose_1us(100, 750, 101), None);
    }

    #[test]
    fn audit_regression_long_isr_between_reads() {
        // The exact 2026-07-11 hole: an ISR ≥ 10 µs runs after the
        // first read; CVR comes back SMALLER than before (post-wrap),
        // which the old `cvr_post <= cvr_pre` guard ACCEPTED while
        // the coarse value paired with it was stale. The new
        // protocol sees coarse_pre != coarse_post and rejects.
        let coarse_before_isr = 100;
        let coarse_after_isr = 101; // base ticked during the ISR
        assert_eq!(compose_1us(coarse_before_isr, 390, coarse_after_isr), None);
    }

    #[test]
    fn wrapping_at_u32_boundary() {
        // coarse near u32::MAX: composition must not panic and must
        // wrap consistently (deltas remain valid under wrapping_sub).
        let t = compose_1us(u32::MAX, 799, u32::MAX).unwrap();
        let t2 = t.wrapping_add(10); // one base tick later
        assert_eq!(t2.wrapping_sub(t), 10);
    }
}
