//! Host-testable capture-source lifecycle, NOT a commutation controller.
//! Hardware integration must serialize each operation and preserve CC2IF until
//! the reference explicitly clears it. No capture timestamp substitutes CNT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u32);
pub struct Source {
    epoch: u32,
    active: bool,
    enabled: bool,
    pending: bool,
    pub overcaptures: u32,
}
impl Source {
    pub const fn new() -> Self {
        Self {
            epoch: 0,
            active: false,
            enabled: false,
            pending: false,
            overcaptures: 0,
        }
    }
    pub fn stop(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.active = false;
        self.enabled = false;
        self.pending = false;
    }
    pub fn phase(&mut self) -> Ticket {
        self.stop();
        self.active = true;
        self.overcaptures = 0;
        Ticket(self.epoch)
    }
    pub fn enable(&mut self, ticket: Ticket) -> bool {
        if !self.active || ticket.0 != self.epoch {
            return false;
        }
        self.enabled = true;
        true
    }
    pub fn capture(&mut self, ticket: Ticket) -> bool {
        if !self.active || ticket.0 != self.epoch {
            return false;
        }
        if self.pending {
            self.overcaptures = self.overcaptures.saturating_add(1);
        }
        self.pending = true;
        true
    }
    pub fn pending(&self) -> bool {
        self.active && self.pending
    }
    pub fn enabled(&self) -> bool {
        self.active && self.enabled
    }
    pub fn valid(&self, ticket: Ticket) -> bool {
        self.active && ticket.0 == self.epoch
    }
    pub fn dispatch(&self) -> bool {
        self.enabled() && self.pending()
    }
    pub fn clear(&mut self) {
        self.pending = false;
    }
    pub fn mask(&mut self) {
        self.enabled = false;
    }
}

/// TIM2 flags are rc_w0: write ones to PRESERVE unrelated flags, never RMW SR.
pub const CC2_FLAGS: u32 = (1 << 2) | (1 << 10);
pub const CLEAR_CC2: u32 = !CC2_FLAGS;
pub const CC2_IE: u32 = 1 << 2;
pub const CC2_ENABLE: u32 = 1 << 4;
/// VALUE bit30 is the live comparator result, not restored configuration.
pub fn same_comp_config(before: u32, after: u32) -> bool {
    (before ^ after) & !(1 << 30) == 0
}
pub fn edge_bits(rising: bool) -> u32 {
    CC2_ENABLE | if rising { 0 } else { 1 << 5 }
}

#[cfg(test)]
mod register_tests {
    use super::*;
    #[test]
    fn clear_preserves_every_unrelated_status_bit() {
        for bit in 0..32 {
            let flag = 1u32 << bit;
            assert_eq!(
                flag & CLEAR_CC2,
                if flag & CC2_FLAGS != 0 { 0 } else { flag }
            );
        }
        assert_eq!(CC2_IE, 4);
    }
    #[test]
    fn polarity_only_uses_cc2_enable_and_polarity() {
        assert_eq!(edge_bits(true), 0x10);
        assert_eq!(edge_bits(false), 0x30);
        for rising in [true, false] {
            assert_eq!(edge_bits(rising) & !0xf0, 0);
        }
    }
    #[test]
    fn masked_phase_ticket_survives_but_stopped_ticket_does_not() {
        let mut s = Source::new();
        let t = s.phase();
        s.mask();
        assert!(s.valid(t));
        s.stop();
        assert!(!s.valid(t));
        let fresh = s.phase();
        assert!(!s.valid(t));
        assert!(s.valid(fresh));
    }
    #[test]
    fn comparator_restoration_excludes_only_output_status() {
        assert!(same_comp_config(0x40000281, 0x281));
        for bit in 0..32 {
            assert_eq!(same_comp_config(0x281, 0x281 ^ (1 << bit)), bit == 30);
        }
    }
}
