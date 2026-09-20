//! One stopped segment retained across observer reset. No drive authority.
//! Capture only after IRQ writers stop; use owned copies, not borrowed statics.
use super::{event_stats::Stats, event_tail::Tail, event_timeline::Timeline};

pub struct StoppedSegment {
    /// reason,stop_us,commits,feedback_scans,peak_raw,bus_min,
    /// guard_isr_max,commit_max,veto (powered guard clock).
    pub power: [u32; 9],
    pub end_us: u32, // observer clock, distinct origin from power.stop_us
    pub stats: Stats,
    pub timeline: Timeline,
    pub prefix: [[u16; 4]; 32],
    pub prefix_len: usize,
    pub tail: Tail,
}
impl StoppedSegment {
    pub fn valid(&self) -> bool {
        self.timeline.total() == self.stats.events
            && self.prefix_len <= 32
            && self.stats.events
                == self.prefix_len as u32 + self.tail.len() as u32 + self.tail.skipped()
    }
}
pub struct Archive {
    record: Option<StoppedSegment>,
}
impl Archive {
    pub const fn new() -> Self {
        Self { record: None }
    }
    /// Refuse overwrite and an unconfirmed stop. Explicit errors let the caller
    /// abort re-entry instead of hiding first-segment evidence loss.
    pub fn freeze(
        &mut self,
        record: StoppedSegment,
        writers_stopped: bool,
        outputs_off: bool,
    ) -> Result<(), u8> {
        if self.record.is_some() {
            return Err(1);
        }
        if !writers_stopped || !outputs_off {
            return Err(2);
        }
        if !record.valid() {
            return Err(3);
        }
        self.record = Some(record);
        Ok(())
    }
    pub fn get(&self) -> Option<&StoppedSegment> {
        self.record.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn recorded() -> StoppedSegment {
        let mut stats = Stats::new();
        let mut prefix = [[0; 4]; 32];
        let mut tail = Tail::new();
        let mut timeline = Timeline::new(60_000_000).unwrap();
        for n in 0..74367u32 {
            let us = 807 + n * 807;
            let step = (n % 6 + 1) as u8;
            let row = [us as u16, (us >> 16) as u16, step as u16, 1667];
            stats.push(us, step);
            timeline.push(us);
            if n < 32 {
                prefix[n as usize] = row;
            } else {
                tail.push(row);
            }
        }
        StoppedSegment {
            power: [8, 60_000_000, 74368, 319175, 337, 10817, 16, 18, 0],
            end_us: 60_000_044,
            stats,
            timeline,
            prefix,
            prefix_len: 32,
            tail,
        }
    }
    #[test]
    fn reset_of_live_buffers_cannot_change_owned_first_segment() {
        let mut live = recorded();
        let mut archive = Archive::new();
        let saved = StoppedSegment {
            power: live.power,
            end_us: live.end_us,
            stats: live.stats,
            timeline: live.timeline,
            prefix: live.prefix,
            prefix_len: live.prefix_len,
            tail: live.tail,
        };
        assert_eq!(archive.freeze(saved, true, true), Ok(()));
        live.stats = Stats::new();
        live.tail = Tail::new();
        live.prefix = [[0; 4]; 32];
        live.power = [0; 9];
        assert_eq!(live.stats.events, 0);
        assert_eq!(live.tail.len(), 0);
        assert_eq!(live.prefix[0], [0; 4]);
        assert_eq!(live.power, [0; 9]);
        let first = archive.get().unwrap();
        assert_eq!(first.stats.events, 74367);
        assert_eq!(first.power[0], 8);
        assert_eq!(first.tail.skipped(), 74303);
        assert_eq!(first.prefix[0][0], 807);
        let last = first.tail.row(31);
        assert_eq!(last[0] as u32 | ((last[1] as u32) << 16), 807 + 74366 * 807);
        assert_eq!(archive.freeze(recorded(), true, true), Err(1));
    }
    #[test]
    fn unsafe_or_incomplete_snapshot_cannot_be_promoted() {
        for (stopped, off) in [(false, true), (true, false), (false, false)] {
            assert_eq!(Archive::new().freeze(recorded(), stopped, off), Err(2));
        }
        let mut bad = recorded();
        bad.prefix_len = 31;
        assert_eq!(Archive::new().freeze(bad, true, true), Err(3));
    }
}
