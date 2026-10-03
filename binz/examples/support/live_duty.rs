//! Request preparation and coherent preload transaction, not motor authority.
//! Caller must hold the guard/write critical section and revalidate ownership,
//! fault, deadlines, feedback and nFAULT. No deadline or controller reset here.
//! Not linked to hardware until timer semantics and disabled timing are checked.
pub const MIN: u32 = super::duty_envelope::MIN;
pub const MAX: u32 = super::duty_envelope::MAX;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prepared {
    ticks: u32,
    duty: u32,
    compare: u32,
}
impl Prepared {
    /// Foreground only: division is intentionally outside the masked writer.
    pub const fn new(ticks: u32, duty: u32) -> Option<Self> {
        if !matches!(ticks, 6400 | 3200 | 2666 | 2000 | 1600 | 1333) || duty < MIN || duty > MAX {
            return None;
        }
        Some(Self {
            ticks,
            duty,
            compare: ticks * duty / 1000,
        })
    }
    pub const fn duty(self) -> u32 {
        self.duty
    }
    pub const fn ticks(self) -> u32 {
        self.ticks
    }
    pub const fn compare(self) -> u32 {
        self.compare
    }
}

pub trait Registers {
    /// Verify live geometry/preload configuration and update generation enabled.
    /// This is NOT a substitute for the caller's fresh electrical/owner guard.
    fn ready(&self, ticks: u32) -> bool;
    /// Suppress natural update transfers without stopping/reinitializing CNT.
    fn suppress_updates(&mut self);
    fn compare(&mut self, channel: usize, value: u32);
    /// Restore natural update generation; MUST NOT issue EGR.UG.
    fn resume_updates(&mut self);
}

/// No divisions, polling waits, phase changes, MOE writes or synthetic COM.
/// ISR exclusion must cover this call AND publishing matching duty metadata.
pub fn apply<R: Registers>(io: &mut R, request: Prepared) -> bool {
    if !io.ready(request.ticks) {
        return false;
    }
    io.suppress_updates();
    io.compare(0, request.compare);
    io.compare(1, request.compare);
    io.compare(2, request.compare);
    io.resume_updates();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Timer {
        ready: bool,
        blocked: bool,
        preload: [u32; 3],
        active: [u32; 3],
        new: u32,
        wrap_mask: u8,
        operation: u8,
        transfers: u8,
    }
    impl Timer {
        fn natural_wrap(&mut self) {
            if !self.blocked {
                self.active = self.preload;
                self.transfers += 1;
            }
            assert!(
                self.active == [224; 3] || self.active == [self.new; 3],
                "mixed CCR transfer"
            );
        }
        fn edge(&mut self) {
            if self.wrap_mask & (1 << self.operation) != 0 {
                self.natural_wrap();
            }
            self.operation += 1;
        }
    }
    impl Registers for Timer {
        fn ready(&self, ticks: u32) -> bool {
            self.ready && !self.blocked && ticks == 3200
        }
        fn suppress_updates(&mut self) {
            self.edge();
            self.blocked = true;
            self.edge();
        }
        fn compare(&mut self, ch: usize, value: u32) {
            self.preload[ch] = value;
            self.edge();
        }
        fn resume_updates(&mut self) {
            self.blocked = false;
            self.edge();
        }
    }
    #[test]
    fn every_valid_duty_and_timer_geometry_is_exact_and_bounded() {
        for ticks in [6400, 3200, 2666, 2000, 1600, 1333] {
            for duty in MIN..=MAX {
                let p = Prepared::new(ticks, duty).unwrap();
                assert_eq!(p.duty(), duty);
                assert_eq!(p.compare(), ticks * duty / 1000);
                assert!(p.compare() * 1000 <= ticks * MAX);
            }
        }
        for ticks in [0, 1332, 1334, 1999, 2001, 3199, 3201, u32::MAX] {
            assert_eq!(Prepared::new(ticks, 70), None);
        }
        for duty in [0, MIN - 1, MAX + 1, u32::MAX] {
            assert_eq!(Prepared::new(3200, duty), None);
        }
    }
    #[test]
    fn any_combination_of_wraps_around_writes_stays_coherent() {
        for duty in [MIN, 70, 80, 100, MAX] {
            for wrap_mask in 0..64 {
                let request = Prepared::new(3200, duty).unwrap();
                let mut t = Timer {
                    ready: true,
                    blocked: false,
                    preload: [224; 3],
                    active: [224; 3],
                    new: request.compare(),
                    wrap_mask,
                    operation: 0,
                    transfers: 0,
                };
                assert!(apply(&mut t, request));
                assert!(!t.blocked);
                assert_eq!(t.preload, [request.compare(); 3]);
                t.natural_wrap();
                assert_eq!(t.active, [request.compare(); 3]);
            }
        }
    }
    #[test]
    fn invalid_geometry_or_preexisting_suppression_never_writes() {
        for (ready, blocked, ticks) in [
            (false, false, 3200),
            (true, true, 3200),
            (true, false, 2666),
        ] {
            let mut t = Timer {
                ready,
                blocked,
                preload: [224; 3],
                active: [224; 3],
                new: 256,
                wrap_mask: 63,
                operation: 0,
                transfers: 0,
            };
            assert!(!apply(&mut t, Prepared::new(ticks, 80).unwrap()));
            assert_eq!(t.operation, 0);
            assert_eq!(t.preload, [224; 3]);
            assert_eq!(t.blocked, blocked);
        }
    }
}
