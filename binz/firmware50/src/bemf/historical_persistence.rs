//! Test-only oracle: offer_timed from commit71f19504c2a5ae403584c04e56689602490cc7fd.
//! Method body retained verbatim; only its name/visibility changed.
use super::*;

impl<const BLANK_64: u32> ZeroCrossWith<BLANK_64> {
    pub(super) fn historical_offer_timed<T: WaitEstimate, F: FilterPolicy, R: FnMut() -> bool>(
        &mut self,
        count: u32,
        rising: bool,
        advance_level: u32,
        policy: &F,
        mut read_level: R,
    ) -> Outcome {
        // 1. half-cycle gate — strictly greater, matching the reference.
        if count <= self.blanking() {
            self.too_early = self.too_early.wrapping_add(1);
            return Outcome::TooEarly;
        }

        // 2. persistence filter. Bounded loop: `filter_level` is a u8, so this
        //    is at most 255 reads and cannot run away.
        let depth = policy.level(self.average_interval);
        let mut i = 0u8;
        while i < depth {
            if read_level() != rising {
                self.unstable = self.unstable.wrapping_add(1);
                return Outcome::Unstable;
            }
            i += 1;
        }

        // 3. Identical blend/publication for both policies; wait may use the
        // preblend estimate, including the seed on the very first acceptance.
        let previous = self.average_interval;
        self.prev_zc = self.last_zc;
        self.last_zc = count;
        self.average_interval = self.clamp_interval(blend_interval(self.average_interval, self.prev_zc, self.last_zc));
        let scheduled = if T::PREVIOUS { previous } else { self.average_interval };
        let advance_level = T::ADVANCE.unwrap_or(advance_level);
        let advance = advance_of(scheduled, advance_level);
        let wait = wait_time(scheduled, advance_level);
        self.accepted = self.accepted.wrapping_add(1);
        Outcome::Accepted {
            wait,
            average_interval: self.average_interval,
            advance,
        }
    }
}
