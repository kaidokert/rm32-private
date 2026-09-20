//! Software provenance for a single board/boot's CSA baseline.
//! No GPIO access, output authority, offset application, or stationarity claim.
//! The live adapter must be a singleton and serialize every ENABLE write and
//! token operation together; constructing a replacement tracker is not reset.

#[derive(Debug)]
pub struct Token {
    generation: u32,
}

pub struct Epoch {
    generation: u32,
    enabled: bool,
    exhausted: bool,
}
impl Epoch {
    pub const fn new() -> Self {
        Self {
            generation: 0,
            enabled: false,
            exhausted: false,
        }
    }
    /// Report commanded ENABLE after performing the physical write. Repeated
    /// high writes (normal COM path) preserve identity; every low revokes it.
    pub fn commanded(&mut self, high: bool) {
        if !high {
            self.enabled = false;
            if let Some(next) = self.generation.checked_add(1) {
                self.generation = next;
            } else {
                self.exhausted = true;
            }
        } else if !self.exhausted {
            self.enabled = true;
        }
    }
    /// Start baseline acquisition only with healthy physical readbacks and all
    /// gates disabled. This does NOT establish motor standstill or CSA settling.
    pub fn begin(&self, enable_high: bool, no_fault: bool, gates_off: bool) -> Option<Token> {
        if self.enabled && !self.exhausted && enable_high && no_fault && gates_off {
            Some(Token {
                generation: self.generation,
            })
        } else {
            None
        }
    }
    /// Check both ends of acquisition and every use of its resulting baseline.
    /// Failed physical readback revokes identity even if it later recovers.
    pub fn matches(&mut self, token: &Token, enable_high: bool, no_fault: bool) -> bool {
        if !enable_high || !no_fault {
            self.invalidate();
            return false;
        }
        self.enabled && !self.exhausted && token.generation == self.generation
    }
    /// Any unobserved enable transition, driver fault, reset, or lost ownership
    /// invalidates the baseline. This grants no permission to drive afterward.
    pub fn invalidate(&mut self) {
        self.commanded(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cannot_start_before_wake_or_without_readbacks() {
        let mut e = Epoch::new();
        assert!(e.begin(true, true, true).is_none());
        e.commanded(true);
        for (en, nf, off) in [
            (false, true, true),
            (true, false, true),
            (true, true, false),
        ] {
            assert!(e.begin(en, nf, off).is_none());
        }
        assert!(e.begin(true, true, true).is_some());
    }
    #[test]
    fn repeated_high_commits_preserve_initial_epoch() {
        let mut e = Epoch::new();
        e.commanded(true);
        let token = e.begin(true, true, true).unwrap();
        for _ in 0..10000 {
            e.commanded(true);
            assert!(e.matches(&token, true, true));
        }
    }
    #[test]
    fn recovery_wake_cannot_revalidate_initial_baseline() {
        let mut e = Epoch::new();
        e.commanded(true);
        let old = e.begin(true, true, true).unwrap();
        e.commanded(false);
        assert!(!e.matches(&old, true, true));
        e.commanded(true);
        assert!(!e.matches(&old, true, true));
        let new = e.begin(true, true, true).unwrap();
        assert!(e.matches(&new, true, true));
    }
    #[test]
    fn disable_during_scan_invalidates_completion() {
        let mut e = Epoch::new();
        e.commanded(true);
        let token = e.begin(true, true, true).unwrap();
        e.commanded(false);
        e.commanded(false);
        e.commanded(true);
        assert!(!e.matches(&token, true, true));
    }
    #[test]
    fn physical_fault_and_explicit_invalidation_are_not_ignored() {
        let mut e = Epoch::new();
        e.commanded(true);
        let token = e.begin(true, true, true).unwrap();
        assert!(!e.matches(&token, false, true));
        assert!(!e.matches(&token, true, false));
        assert!(!e.matches(&token, true, true));
        e.invalidate();
        e.commanded(true);
        assert!(!e.matches(&token, true, true));
    }
    #[test]
    fn overflow_is_permanent_refusal_not_token_reuse() {
        let mut e = Epoch {
            generation: u32::MAX,
            enabled: true,
            exhausted: false,
        };
        let token = e.begin(true, true, true).unwrap();
        e.commanded(false);
        e.commanded(true);
        assert!(!e.matches(&token, true, true));
        assert!(e.begin(true, true, true).is_none());
        e.commanded(false);
        e.commanded(true);
        assert!(e.begin(true, true, true).is_none());
    }
}
