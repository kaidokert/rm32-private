//! Ownership checks for circular DMA with two five-word ADC scan halves.
//! Hardware integration contract: exactly one five-channel scan per101us;
//! circular NDTR=10, no other flag clearer, no ADC/DMA reconfiguration without
//! changing epoch. Volatile copy and compiler fences belong to the caller.
//! This policy does not itself stop DMA, clear flags, or publish feedback.
pub const TC: u32 = 1 << 1;
pub const HT: u32 = 1 << 2;
pub const TE: u32 = 1 << 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Transfer,
    AmbiguousFlags,
    Position,
    Deadline,
    Restarted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lease {
    half: u8,
    remaining: u16,
    epoch: u32,
}
impl Lease {
    /// Start timing BEFORE reading flags/NDTR; all register reads through final
    /// verification must fall within100us. Reject both pending boundaries:
    /// hardware flags are latches, not counters of completed scans.
    pub fn begin(flags: u32, remaining: u16, epoch: u32) -> Result<Self, Fault> {
        if flags & TE != 0 {
            return Err(Fault::Transfer);
        }
        let half = match flags & (HT | TC) {
            HT => 0,
            TC => 1,
            _ => return Err(Fault::AmbiguousFlags),
        };
        if !opposite_half(half, remaining) {
            return Err(Fault::Position);
        }
        Ok(Self {
            half,
            remaining,
            epoch,
        })
    }
    pub fn word_offset(self) -> usize {
        self.half as usize * 5
    }
    pub fn acknowledge_flag(self) -> u32 {
        if self.half == 0 { HT } else { TC }
    }
    /// Caller clears ONLY acknowledge_flag before copying five volatile words.
    /// Read NDTR then flags after the copy, then end timestamp; publish only
    /// on Ok. Local copied words remain untrusted until this succeeds.
    pub fn finish(
        self,
        flags_after: u32,
        remaining_after: u16,
        elapsed_us: u32,
        epoch: u32,
    ) -> Result<(), Fault> {
        if epoch != self.epoch {
            return Err(Fault::Restarted);
        }
        if elapsed_us > 100 {
            return Err(Fault::Deadline);
        }
        if flags_after & TE != 0 {
            return Err(Fault::Transfer);
        }
        if flags_after & (HT | TC) != 0 {
            return Err(Fault::AmbiguousFlags);
        }
        if !opposite_half(self.half, remaining_after) || remaining_after > self.remaining {
            return Err(Fault::Position);
        }
        Ok(())
    }
}
fn opposite_half(completed: u8, remaining: u16) -> bool {
    match completed {
        0 => (1..=5).contains(&remaining),
        1 => (6..=10).contains(&remaining),
        _ => false,
    }
}

/// Ascending hardware channels0,1,4,6,13 -> logicalA,B,C,bus,VREF.
pub fn logical(raw: [u16; 5]) -> Option<[u16; 5]> {
    if raw.iter().any(|v| *v > 4095) {
        return None;
    }
    Some([raw[2], raw[1], raw[0], raw[3], raw[4]])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copy_other_half_and_remap() {
        let a = Lease::begin(HT, 5, 8).unwrap();
        assert_eq!(a.word_offset(), 0);
        assert_eq!(a.acknowledge_flag(), HT);
        assert_eq!(a.finish(0, 1, 99, 8), Ok(()));
        let b = Lease::begin(TC, 10, 8).unwrap();
        assert_eq!(b.word_offset(), 5);
        assert_eq!(b.acknowledge_flag(), TC);
        assert_eq!(b.finish(0, 6, 100, 8), Ok(()));
        assert_eq!(
            logical([100, 200, 300, 1000, 1500]),
            Some([300, 200, 100, 1000, 1500])
        );
        assert_eq!(logical([65535, 0, 0, 0, 0]), None);
    }
    #[test]
    fn both_flags_are_not_two_safe_snapshots() {
        assert_eq!(Lease::begin(HT | TC, 5, 1), Err(Fault::AmbiguousFlags));
        assert_eq!(Lease::begin(0, 5, 1), Err(Fault::AmbiguousFlags));
        assert_eq!(Lease::begin(TE | HT, 5, 1), Err(Fault::Transfer));
    }
    #[test]
    fn wrap_midcopy_or_hidden_long_pause_is_rejected() {
        let a = Lease::begin(HT, 5, 1).unwrap();
        assert_eq!(a.finish(TC, 10, 20, 1), Err(Fault::AmbiguousFlags));
        assert_eq!(a.finish(0, 10, 20, 1), Err(Fault::Position));
        assert_eq!(a.finish(0, 5, 202, 1), Err(Fault::Deadline));
        assert_eq!(a.finish(0, 5, 2, 2), Err(Fault::Restarted));
        assert_eq!(a.finish(TE, 4, 2, 1), Err(Fault::Transfer));
    }
    #[test]
    fn all_positions_enforce_completed_half_ownership() {
        for remaining in 0..=11 {
            assert_eq!(
                Lease::begin(HT, remaining, 0).is_ok(),
                (1..=5).contains(&remaining)
            );
            assert_eq!(
                Lease::begin(TC, remaining, 0).is_ok(),
                (6..=10).contains(&remaining)
            );
        }
        let a = Lease::begin(HT, 2, 0).unwrap();
        assert_eq!(a.finish(0, 3, 1, 0), Err(Fault::Position));
    }
}
