//! The live in-run host command parser.
//!
//! This is the grammar that is active **while the motor is powered**, and it is
//! deliberately far stricter than the idle shell: it accepts exactly one
//! command shape and treats everything else as a stop request.
//!
//! The design rule is that every ambiguity resolves to **stop**:
//!
//! * five bytes are unconditional immediate stops, from any parser state;
//! * an unrecognized byte is a stop, not a discarded character;
//! * a `du` command with no digits, a zero value, or a value above the
//!   envelope ceiling is a stop, not a clamp;
//! * a *partially typed* command that goes stale is a stop, not a reset —
//!   a host that died mid-command must not leave the motor running.
//!
//! That last rule is the one worth stating twice: the timeout does not discard
//! the fragment and carry on, it stops the motor. A dropped serial link is
//! indistinguishable from a host that stopped caring, so it is treated as the
//! latter.
//!
//! Division-free and panic-free; it is fed from the USART interrupt.

/// A partially typed command older than this stops the run.
pub const COMMAND_TIMEOUT_US: u32 = 250_000;

/// Maximum digits accepted in a `du` value.
pub const MAX_DIGITS: u8 = 3;

/// What the host asked for.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action {
    /// Nothing to do yet (mid-command, or an empty line).
    None,
    /// Stop the motor. Every ambiguous or malformed input lands here.
    Stop,
    /// Apply this duty, in tenths of a percent.
    Duty(u16),
    /// Emit the status line.
    Status,
}

/// Bytes that stop the run immediately from any state.
///
/// `o` and `s` are the typed words, `!` is the panic key, `0x03` is Ctrl-C and
/// `0x1B` is ESC — so both "I typed stop" and "I hit the interrupt key" work.
pub const STOP_BYTES: [u8; 5] = [b'o', b's', b'!', 0x03, 0x1B];

#[inline]
const fn is_stop_byte(b: u8) -> bool {
    b == b'o' || b == b's' || b == b'!' || b == 0x03 || b == 0x1B
}

#[inline]
const fn is_eol(b: u8) -> bool {
    b == b'\r' || b == b'\n'
}

/// Parser states, named so the transition tests read as behavior.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum State {
    /// Between commands.
    Idle,
    /// Saw `d`, expecting `u`.
    SawD,
    /// Saw `du`, accumulating digits.
    Digits,
}

/// The live command parser.
///
/// `MAX_TENTHS` is the envelope ceiling as a type parameter, so a parser built
/// for one duty personality cannot admit another's range.
pub struct Parser<const MAX_TENTHS: u16> {
    state: State,
    value: u16,
    digits: u8,
    started: u32,
}

impl<const MAX_TENTHS: u16> Default for Parser<MAX_TENTHS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const MAX_TENTHS: u16> Parser<MAX_TENTHS> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: State::Idle,
            value: 0,
            digits: 0,
            started: 0,
        }
    }

    #[inline]
    fn clear(&mut self) {
        self.state = State::Idle;
        self.value = 0;
        self.digits = 0;
    }

    /// True while a command is partially typed.
    #[inline]
    pub fn pending(&self) -> bool {
        self.state != State::Idle
    }

    /// Age out a stale partial command. Call this from the control tick as
    /// well as before each byte, so a host that stops transmitting mid-command
    /// is noticed even though no further byte ever arrives.
    #[must_use = "a dropped Action discards a stop request"]
    pub fn poll(&mut self, now_us: u32) -> Action {
        if self.state != State::Idle && now_us.wrapping_sub(self.started) >= COMMAND_TIMEOUT_US {
            self.clear();
            return Action::Stop;
        }
        Action::None
    }

    /// Feed one received byte.
    #[must_use = "a dropped Action discards a stop request"]
    pub fn byte(&mut self, b: u8, now_us: u32) -> Action {
        // A byte that arrives after the fragment already went stale does not
        // rescue it.
        if self.poll(now_us) == Action::Stop {
            return Action::Stop;
        }
        if is_stop_byte(b) {
            self.clear();
            return Action::Stop;
        }
        match self.state {
            State::Idle => {
                if b == b'd' {
                    self.state = State::SawD;
                    self.value = 0;
                    self.digits = 0;
                    self.started = now_us;
                    Action::None
                } else if b == b'?' {
                    Action::Status
                } else if is_eol(b) {
                    Action::None
                } else {
                    Action::Stop
                }
            }
            State::SawD => {
                if b == b'u' {
                    self.state = State::Digits;
                    Action::None
                } else {
                    self.clear();
                    Action::Stop
                }
            }
            State::Digits => {
                if b.is_ascii_digit() && self.digits < MAX_DIGITS {
                    // `digits < 3` bounds the value at 999, so this cannot
                    // overflow a u16.
                    self.value = self.value * 10 + (b - b'0') as u16;
                    self.digits += 1;
                    if self.value > MAX_TENTHS {
                        self.clear();
                        return Action::Stop;
                    }
                    Action::None
                } else if is_eol(b) && self.digits > 0 {
                    let v = self.value;
                    self.clear();
                    if v == 0 { Action::Stop } else { Action::Duty(v) }
                } else {
                    self.clear();
                    Action::Stop
                }
            }
        }
    }

    /// Feed a whole byte sequence, returning the last non-`None` action.
    /// Convenience for tests and for draining a receive buffer.
    #[must_use = "a dropped Action discards a stop request"]
    pub fn feed(&mut self, bytes: &[u8], now_us: u32) -> Action {
        let mut last = Action::None;
        let mut i = 0;
        while i < bytes.len() {
            let a = self.byte(bytes[i], now_us);
            if a != Action::None {
                last = a;
            }
            if a == Action::Stop {
                break;
            }
            i += 1;
        }
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duty::ENVELOPE_MAX;

    type P = Parser<ENVELOPE_MAX>;

    fn parser() -> P {
        P::new()
    }

    // --- the happy path ----------------------------------------------------

    #[test]
    fn a_well_formed_duty_command_is_accepted() {
        let mut p = parser();
        assert_eq!(p.feed(b"du100\r", 0), Action::Duty(100));
        assert!(!p.pending(), "parser must return to idle");
    }

    #[test]
    fn the_reference_duty_commands_all_parse() {
        for (bytes, want) in [
            (&b"du40\r"[..], 40u16),
            (&b"du100\r"[..], 100),
            (&b"du250\r"[..], 250),
            (&b"du300\r"[..], 300),
        ] {
            let mut p = parser();
            assert_eq!(p.feed(bytes, 0), Action::Duty(want), "{bytes:?}");
        }
    }

    #[test]
    fn newline_terminates_as_well_as_carriage_return() {
        let mut p = parser();
        assert_eq!(p.feed(b"du100\n", 0), Action::Duty(100));
    }

    #[test]
    fn a_status_query_is_answered_without_touching_the_drive() {
        let mut p = parser();
        assert_eq!(p.byte(b'?', 0), Action::Status);
        assert!(!p.pending());
    }

    #[test]
    fn a_bare_end_of_line_is_ignored() {
        let mut p = parser();
        assert_eq!(p.byte(b'\r', 0), Action::None);
        assert_eq!(p.byte(b'\n', 0), Action::None);
    }

    #[test]
    fn successive_commands_parse_independently() {
        let mut p = parser();
        assert_eq!(p.feed(b"du100\r", 0), Action::Duty(100));
        assert_eq!(p.feed(b"du250\r", 1000), Action::Duty(250));
        assert_eq!(p.feed(b"du40\r", 2000), Action::Duty(40));
    }

    // --- immediate stop bytes ----------------------------------------------

    #[test]
    fn every_stop_byte_stops_from_idle() {
        for b in STOP_BYTES {
            let mut p = parser();
            assert_eq!(p.byte(b, 0), Action::Stop, "byte {b:#04x}");
        }
    }

    #[test]
    fn every_stop_byte_stops_from_every_partial_state() {
        // Mid-`d`, mid-`du`, and mid-digits.
        for prefix in [&b"d"[..], &b"du"[..], &b"du1"[..], &b"du12"[..]] {
            for b in STOP_BYTES {
                let mut p = parser();
                let _ = p.feed(prefix, 0);
                assert_eq!(p.byte(b, 0), Action::Stop, "prefix {prefix:?} byte {b:#04x}");
                assert!(!p.pending(), "stop must clear the fragment");
            }
        }
    }

    #[test]
    fn ctrl_c_and_escape_are_stops() {
        let mut p = parser();
        assert_eq!(p.byte(0x03, 0), Action::Stop);
        let mut p2 = parser();
        assert_eq!(p2.byte(0x1B, 0), Action::Stop);
    }

    // --- malformed input fails closed --------------------------------------

    #[test]
    fn an_unrecognized_leading_byte_is_a_stop_not_a_discard() {
        for b in [b'x', b'z', b'D', b'U', b'1', b' ', 0u8, 0xFF] {
            let mut p = parser();
            assert_eq!(p.byte(b, 0), Action::Stop, "byte {b:#04x}");
        }
    }

    #[test]
    fn a_d_not_followed_by_u_is_a_stop() {
        for b in [b'x', b'd', b'1', b'\r', b'?'] {
            let mut p = parser();
            assert_eq!(p.byte(b'd', 0), Action::None);
            assert_eq!(p.byte(b, 0), Action::Stop, "after d: {b:#04x}");
        }
    }

    #[test]
    fn a_du_with_no_digits_is_a_stop() {
        let mut p = parser();
        assert_eq!(p.feed(b"du\r", 0), Action::Stop);
    }

    #[test]
    fn a_non_digit_inside_the_value_is_a_stop() {
        let mut p = parser();
        assert_eq!(p.feed(b"du1x\r", 0), Action::Stop);
    }

    #[test]
    fn a_fourth_digit_is_a_stop_rather_than_a_truncation() {
        let mut p = parser();
        // 3 digits max; a 4th must not silently reinterpret the value
        assert_eq!(p.feed(b"du1000\r", 0), Action::Stop);
    }

    #[test]
    fn a_zero_duty_command_is_a_stop() {
        // Distinct from the envelope's "0 means stop driving": a host that
        // explicitly asks for zero while powered is asking to stop.
        let mut p = parser();
        assert_eq!(p.feed(b"du0\r", 0), Action::Stop);
        let mut p2 = parser();
        assert_eq!(p2.feed(b"du000\r", 0), Action::Stop);
    }

    #[test]
    fn a_duty_above_the_ceiling_is_a_stop_not_a_clamp() {
        let mut p = parser();
        assert_eq!(p.feed(b"du999\r", 0), Action::Stop);
        // and it is refused as soon as the value exceeds the ceiling, without
        // waiting for the terminator
        let mut p2 = parser();
        assert_eq!(p2.byte(b'd', 0), Action::None);
        assert_eq!(p2.byte(b'u', 0), Action::None);
        assert_eq!(p2.byte(b'9', 0), Action::None); // 9
        assert_eq!(p2.byte(b'9', 0), Action::None); // 99
        assert_eq!(p2.byte(b'9', 0), Action::Stop, "999 > ceiling");
    }

    #[test]
    fn exactly_the_ceiling_is_accepted() {
        let mut p = parser();
        let mut buf = [0u8; 8];
        let s = ENVELOPE_MAX;
        let text = [
            b'd',
            b'u',
            b'0' + (s / 100) as u8,
            b'0' + (s / 10 % 10) as u8,
            b'0' + (s % 10) as u8,
            b'\r',
        ];
        buf[..6].copy_from_slice(&text);
        assert_eq!(p.feed(&buf[..6], 0), Action::Duty(ENVELOPE_MAX));
    }

    // --- the timeout -------------------------------------------------------

    #[test]
    fn a_partial_command_going_stale_stops_the_motor() {
        // The critical rule: a host that dies mid-command must not leave the
        // motor running.
        let mut p = parser();
        assert_eq!(p.byte(b'd', 1_000), Action::None);
        assert_eq!(p.byte(b'u', 1_000), Action::None);
        assert!(p.pending());
        assert_eq!(p.poll(1_000 + COMMAND_TIMEOUT_US), Action::Stop);
        assert!(!p.pending());
    }

    #[test]
    fn the_timeout_is_measured_from_the_start_of_the_command() {
        let mut p = parser();
        let _ = p.byte(b'd', 5_000);
        // just inside
        assert_eq!(p.poll(5_000 + COMMAND_TIMEOUT_US - 1), Action::None);
        assert!(p.pending(), "must still be accumulating");
        // and exactly at the limit it trips
        assert_eq!(p.poll(5_000 + COMMAND_TIMEOUT_US), Action::Stop);
    }

    #[test]
    fn a_late_byte_does_not_rescue_a_stale_fragment() {
        let mut p = parser();
        let _ = p.byte(b'd', 0);
        let _ = p.byte(b'u', 0);
        let _ = p.byte(b'1', 0);
        // the terminator arrives too late
        assert_eq!(p.byte(b'\r', COMMAND_TIMEOUT_US), Action::Stop);
    }

    #[test]
    fn an_idle_parser_never_times_out() {
        let mut p = parser();
        assert_eq!(p.poll(0), Action::None);
        assert_eq!(p.poll(u32::MAX), Action::None);
        assert_eq!(p.poll(COMMAND_TIMEOUT_US * 10), Action::None);
    }

    #[test]
    fn a_completed_command_leaves_nothing_to_time_out() {
        let mut p = parser();
        assert_eq!(p.feed(b"du100\r", 0), Action::Duty(100));
        assert_eq!(p.poll(COMMAND_TIMEOUT_US * 5), Action::None);
    }

    #[test]
    fn the_timeout_survives_a_timestamp_wrap() {
        let start = u32::MAX - 1_000;
        let mut p = parser();
        let _ = p.byte(b'd', start);
        // still inside the window, on the other side of the wrap
        assert_eq!(p.poll(start.wrapping_add(COMMAND_TIMEOUT_US - 1)), Action::None);
        assert_eq!(p.poll(start.wrapping_add(COMMAND_TIMEOUT_US)), Action::Stop);
    }

    // --- the ceiling is a type parameter -----------------------------------

    #[test]
    fn a_parser_cannot_admit_another_personalitys_range() {
        // 400 tenths is refused by the production parser and admitted by the
        // raised-ceiling one; the difference is checked at compile time.
        let mut prod: Parser<300> = Parser::new();
        assert_eq!(prod.feed(b"du400\r", 0), Action::Stop);
        let mut raised: Parser<500> = Parser::new();
        assert_eq!(raised.feed(b"du400\r", 0), Action::Duty(400));
    }

    // --- no input can wedge the parser -------------------------------------

    #[test]
    fn no_byte_sequence_leaves_the_parser_pending_forever() {
        // Every path either completes, stops, or stays pending with a live
        // timeout — there is no state that ignores both.
        for a in 0u8..=255 {
            for b in [b'u', b'1', b'\r', 0x03, b'x'] {
                let mut p = parser();
                let _ = p.byte(a, 0);
                let _ = p.byte(b, 0);
                if p.pending() {
                    assert_eq!(
                        p.poll(COMMAND_TIMEOUT_US),
                        Action::Stop,
                        "pending after {a:#04x},{b:#04x} but never times out"
                    );
                }
            }
        }
    }

    #[test]
    fn the_value_accumulator_cannot_overflow() {
        // Bounded to 3 digits, so the largest reachable value is 999.
        let mut p: Parser<65535> = Parser::new();
        assert_eq!(p.feed(b"du999\r", 0), Action::Duty(999));
    }
}
