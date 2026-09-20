//! Observe-only capture lifecycle. All firmware accesses must be serialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u32);
pub struct Epoch {
    generation: u32,
    active: bool,
    armed: bool,
}
impl Epoch {
    pub const fn new() -> Self {
        Self {
            generation: 0,
            active: false,
            armed: false,
        }
    }
    pub fn stop(&mut self) {
        self.active = false;
        self.armed = false;
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn prepare(&mut self) {
        self.stop();
        self.active = true;
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn before_mux(&mut self) -> Option<Ticket> {
        self.armed = false;
        self.generation = self.generation.wrapping_add(1);
        if self.active {
            Some(Ticket(self.generation))
        } else {
            None
        }
    }
    pub fn after_mux(&mut self, ticket: Ticket) -> bool {
        if !self.active || ticket.0 != self.generation {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        self.armed = true;
        true
    }
    pub fn before_reset(&mut self) -> bool {
        let sample = self.active && self.armed;
        self.armed = false;
        self.generation = self.generation.wrapping_add(1);
        sample
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normal_and_seed_reset() {
        let mut e = Epoch::new();
        e.prepare();
        assert!(!e.before_reset());
        let t = e.before_mux().unwrap();
        assert!(e.after_mux(t));
        assert!(!e.after_mux(t));
        assert!(e.before_reset());
        assert!(!e.before_reset());
        assert!(!e.after_mux(t));
    }
    #[test]
    fn stop_and_new_session_revoke_ticket() {
        let mut e = Epoch::new();
        e.prepare();
        let t = e.before_mux().unwrap();
        e.stop();
        assert!(!e.after_mux(t));
        assert!(!e.before_reset());
        e.prepare();
        assert!(!e.after_mux(t));
        let fresh = e.before_mux().unwrap();
        assert!(e.after_mux(fresh));
    }
    #[test]
    fn superseding_mux_and_wrap() {
        let mut e = Epoch {
            generation: u32::MAX - 1,
            active: true,
            armed: false,
        };
        let old = e.before_mux().unwrap();
        let fresh = e.before_mux().unwrap();
        assert!(!e.after_mux(old));
        assert!(e.after_mux(fresh));
        assert!(e.before_reset());
    }
}
