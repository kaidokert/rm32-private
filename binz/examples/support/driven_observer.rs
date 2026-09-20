//! Observe-only wrapper around the existing minz-backed detector.
//! Epoch must be incremented on EVERY physical commutation, not derived from
//! step alone. Caller serializes command/sample snapshots with the scheduler.
//! Microsecond timebase; raw COMP polarity must be inverted by the caller.
//! Neither a command nor an observed candidate grants handoff authority.
use crate::detector::Detector;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    NoSector,
    Epoch,
    Blank,
    Bracket,
    Gap,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub step: u8,
    pub onset_us: u32,
    pub confirmed_us: u32,
}
pub struct Observer {
    detector: Detector,
    sector: Option<(u32, u8, u32)>,
    last: Option<u32>,
    onset: Option<u32>,
    emitted: bool,
    pub commands: u32,
    pub candidates: u32,
    pub missed: u32,
    pub rejected: [u32; 5],
}
impl Observer {
    pub fn new() -> Self {
        Self {
            detector: Detector::default(),
            sector: None,
            last: None,
            onset: None,
            emitted: false,
            commands: 0,
            candidates: 0,
            missed: 0,
            rejected: [0; 5],
        }
    }
    /// Explicit physical epoch prevents step-number ABA after six commands.
    /// Invalid/reordered commands invalidate observation, never fabricate edges.
    pub fn command(&mut self, epoch: u32, step: u8, now: u32) -> bool {
        let valid = (1..=6).contains(&step)
            && self.sector.is_none_or(|(e, s, t)| {
                epoch == e.wrapping_add(1)
                    && step == s % 6 + 1
                    && now.wrapping_sub(t) > 0
                    && now.wrapping_sub(t) < 20_000
            });
        if self.sector.is_some() && !self.emitted {
            self.missed += 1;
        }
        self.detector = Detector::default();
        self.last = None;
        self.onset = None;
        self.emitted = false;
        self.sector = if valid {
            Some((epoch, step, now))
        } else {
            None
        };
        if valid {
            self.commands += 1;
        } else {
            self.rejected[1] += 1;
        }
        valid
    }
    fn reject(&mut self, r: Rejection) -> Result<Option<Candidate>, Rejection> {
        let i = match r {
            Rejection::NoSector => 0,
            Rejection::Epoch => 1,
            Rejection::Blank => 2,
            Rejection::Bracket => 3,
            Rejection::Gap => 4,
        };
        self.rejected[i] += 1;
        self.detector = Detector::default();
        self.onset = None;
        self.last = None;
        // emitted deliberately survives invalid samples: never two per epoch.
        Err(r)
    }
    /// Both epochs bracket the actual comparator read. No ADC duration is
    /// substituted for this bracket. 200us post-commutation blank matches the
    /// existing observation microscope; <=100us sample gaps, <=2us read bracket.
    /// These are diagnostic admission limits, not production filter tuning.
    pub fn sample(
        &mut self,
        before_epoch: u32,
        after_epoch: u32,
        before: u32,
        after: u32,
        hal_level: bool,
    ) -> Result<Option<Candidate>, Rejection> {
        let Some((epoch, step, start)) = self.sector else {
            return self.reject(Rejection::NoSector);
        };
        if before_epoch != epoch || after_epoch != epoch {
            return self.reject(Rejection::Epoch);
        }
        if after.wrapping_sub(before) > 2 || before.wrapping_sub(start) >= 20_000 {
            return self.reject(Rejection::Bracket);
        }
        if before.wrapping_sub(start) < 200 {
            return self.reject(Rejection::Blank);
        }
        if let Some(last) = self.last {
            let gap = before.wrapping_sub(last);
            if gap == 0 || gap > 100 {
                return self.reject(Rejection::Gap);
            }
        }
        self.last = Some(before);
        if self.emitted {
            return Ok(None);
        }
        let d = self.detector.update(step, hal_level, true);
        if !d.armed || hal_level != d.expected {
            self.onset = None;
        } else if self.onset.is_none() {
            self.onset = Some(before);
        }
        if d.event {
            self.emitted = true;
            self.candidates += 1;
            return Ok(Some(Candidate {
                step,
                onset_us: self.onset.unwrap(),
                confirmed_us: after,
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(o: &mut Observer, e: u32, t: u32, l: bool) -> Result<Option<Candidate>, Rejection> {
        o.sample(e, e, t, t + 1, l)
    }
    #[test]
    fn all_sectors_require_real_opposite_then_expected_samples() {
        let mut o = Observer::new();
        for i in 0..18u32 {
            let step = (i % 6 + 1) as u8;
            let base = i * 833;
            let expected = minz_core::drive::edges_for(3, step - 1).0;
            assert!(o.command(i, step, base));
            assert_eq!(sample(&mut o, i, base + 200, expected), Ok(None));
            for (dt, l) in [(250, !expected), (300, !expected), (350, expected)] {
                assert_eq!(sample(&mut o, i, base + dt, l), Ok(None));
            }
            assert_eq!(
                sample(&mut o, i, base + 400, expected),
                Ok(Some(Candidate {
                    step,
                    onset_us: base + 350,
                    confirmed_us: base + 401
                }))
            );
        }
        assert_eq!((o.commands, o.candidates, o.missed), (18, 18, 0));
    }
    #[test]
    fn commanded_sequence_alone_never_qualifies() {
        let mut o = Observer::new();
        for i in 0..24 {
            assert!(o.command(i, (i % 6 + 1) as u8, i * 833));
        }
        assert_eq!((o.commands, o.candidates, o.missed), (24, 0, 23));
    }
    #[test]
    fn stale_epoch_aba_blank_and_read_bracket_refuse() {
        let mut o = Observer::new();
        assert_eq!(sample(&mut o, 0, 200, false), Err(Rejection::NoSector));
        for i in 0..7 {
            assert!(o.command(i, (i % 6 + 1) as u8, i * 833));
        }
        let t = 6 * 833 + 200;
        assert_eq!(sample(&mut o, 0, t, false), Err(Rejection::Epoch));
        assert_eq!(o.sample(6, 7, t, t + 1, false), Err(Rejection::Epoch));
        assert_eq!(sample(&mut o, 6, t - 1, false), Err(Rejection::Blank));
        assert_eq!(o.sample(6, 6, t, t + 3, false), Err(Rejection::Bracket));
        assert_eq!(o.candidates, 0);
    }
    #[test]
    fn gaps_cancel_pending_and_never_duplicate_an_emitted_epoch() {
        let mut o = Observer::new();
        o.command(0, 1, 0);
        for (t, l) in [(200, false), (250, false), (300, true)] {
            sample(&mut o, 0, t, l).unwrap();
        }
        assert_eq!(sample(&mut o, 0, 401, true), Err(Rejection::Gap));
        assert_eq!(sample(&mut o, 0, 450, true), Ok(None));
        for (t, l) in [(500, false), (550, false), (600, true)] {
            sample(&mut o, 0, t, l).unwrap();
        }
        assert!(sample(&mut o, 0, 650, true).unwrap().is_some());
        assert_eq!(sample(&mut o, 0, 751, true), Err(Rejection::Gap));
        for (t, l) in [(800, false), (850, false), (900, true), (950, true)] {
            assert_eq!(sample(&mut o, 0, t, l), Ok(None));
        }
        assert_eq!(o.candidates, 1);
    }
    #[test]
    fn reordered_command_invalidates_and_wrap_is_supported() {
        let mut o = Observer::new();
        let start = u32::MAX - 300;
        assert!(o.command(u32::MAX, 1, start));
        for (dt, l) in [(200, false), (250, false), (300, true), (350, true)] {
            o.sample(
                u32::MAX,
                u32::MAX,
                start.wrapping_add(dt),
                start.wrapping_add(dt + 1),
                l,
            )
            .unwrap();
        }
        assert_eq!(o.candidates, 1);
        assert!(o.command(0, 2, start.wrapping_add(833)));
        assert!(!o.command(2, 3, start.wrapping_add(1666)));
        assert_eq!(sample(&mut o, 2, 2000, true), Err(Rejection::NoSector));
    }
}
