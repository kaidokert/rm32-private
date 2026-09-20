//! Latest coherent timed frame for startup readers; no peripheral authority.
//! DMA producer feeds the average guard for EVERY frame, independently of this
//! deliberately lossy foreground cache. Never average a repeated cached frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub raw: [u16; 5],
    pub acquired: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Missing,
    Stale,
    Reordered,
    Invalid,
    Latched,
}
pub struct Latest {
    frame: Option<Frame>,
    fault: bool,
}
impl Latest {
    pub const fn new() -> Self {
        Self {
            frame: None,
            fault: false,
        }
    }
    /// Caller serializes publish/read and provides ONE shared clock domain.
    /// Acquired is the original trigger stamp, never the delivery timestamp.
    pub fn publish(&mut self, frame: Frame, now: u32) -> Result<(), Error> {
        if self.fault {
            return Err(Error::Latched);
        }
        // The DMA current owner deliberately includes exact phase rails in
        // its signed average and reports their count. This cache must not
        // secretly turn one such sample into a second pulse-current veto.
        // Bus/VREF rails remain invalid, as do impossible >12-bit DMA words.
        let invalid_phase_word = frame.raw[0] > 4095
            || frame.raw[1] > 4095
            || frame.raw[2] > 4095;
        let invalid_reference = frame.raw[3] == 0
            || frame.raw[3] >= 4095
            || frame.raw[4] == 0
            || frame.raw[4] >= 4095;
        let error = if invalid_phase_word || invalid_reference {
            Some(Error::Invalid)
        } else if now.wrapping_sub(frame.acquired) > 1000 {
            Some(Error::Stale)
        } else if self.frame.is_some_and(|old| {
            let delta = frame.acquired.wrapping_sub(old.acquired);
            delta == 0 || delta > 1000
        }) {
            Some(Error::Reordered)
        } else {
            None
        };
        if let Some(error) = error {
            self.fault = true;
            return Err(error);
        }
        self.frame = Some(frame);
        Ok(())
    }
    /// Repeated reads preserve acquisition age. They confer no producer progress.
    pub fn read(&self, now: u32) -> Result<Frame, Error> {
        if self.fault {
            return Err(Error::Latched);
        }
        let frame = self.frame.ok_or(Error::Missing)?;
        if now.wrapping_sub(frame.acquired) > 1000 {
            return Err(Error::Stale);
        }
        Ok(frame)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn f(acquired: u32) -> Frame {
        Frame {
            raw: [2048; 5],
            acquired,
        }
    }
    #[test]
    fn cached_reads_never_refresh_age() {
        let mut s = Latest::new();
        assert_eq!(s.read(0), Err(Error::Missing));
        s.publish(f(100), 150).unwrap();
        for now in 150..=1100 {
            assert_eq!(s.read(now).unwrap().acquired, 100);
        }
        assert_eq!(s.read(1101), Err(Error::Stale));
    }
    #[test]
    fn duplicate_reordered_and_delayed_publication_latch() {
        for stamp in [99, 100, 1101] {
            let mut s = Latest::new();
            s.publish(f(100), 100).unwrap();
            assert_eq!(s.publish(f(stamp), stamp), Err(Error::Reordered));
            assert_eq!(s.publish(f(1302), 1302), Err(Error::Latched));
        }
        let mut s = Latest::new();
        assert_eq!(s.publish(f(100), 1101), Err(Error::Stale));
        assert_eq!(s.read(1101), Err(Error::Latched));
    }
    #[test]
    fn original_timestamps_survive_clock_wrap_and_cache_overwrite() {
        let mut s = Latest::new();
        let first = u32::MAX - 100;
        s.publish(f(first), first + 50).unwrap();
        s.publish(f(100), 150).unwrap();
        assert_eq!(s.read(200).unwrap(), f(100));
    }
    #[test]
    fn phase_rails_are_current_owner_data_not_cache_faults() {
        for channel in 0..3 {
            for value in [0, 4095] {
                let mut s = Latest::new();
                let mut frame = f(1);
                frame.raw[channel] = value;
                assert_eq!(s.publish(frame, 1), Ok(()));
                assert_eq!(s.read(1), Ok(frame));
            }
        }
    }
    #[test]
    fn bus_vref_rails_and_impossible_dma_words_still_latch() {
        for channel in 0..5 {
            let values: &[u16] = if channel < 3 {
                &[u16::MAX]
            } else {
                &[0, 4095, u16::MAX]
            };
            for &value in values {
                let mut s = Latest::new();
                let mut frame = f(1);
                frame.raw[channel] = value;
                assert_eq!(s.publish(frame, 1), Err(Error::Invalid));
                assert_eq!(s.read(1), Err(Error::Latched));
            }
        }
    }
}
