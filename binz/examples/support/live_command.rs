//! Allocation-free foreground parser for opt-in live BEMF exploration.
//! Produces requests only: no registers, guard changes, or run-deadline writes.
//! `du70\r` requests 7%; `?` requests status; off/s/!/ESC/Ctrl-C stop immediately.
//! Any malformed command or a partial command older than 250ms also stops.
pub const MAX_DUTY_TENTHS: u16 = super::duty_envelope::MAX as u16;
const COMMAND_TIMEOUT_US: u32 = 250_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Duty(u16),
    Status,
    Stop,
}
pub struct Parser {
    state: u8,
    digits: u8,
    value: u16,
    started: u32,
}
impl Parser {
    pub const fn new() -> Self {
        Self {
            state: 0,
            digits: 0,
            value: 0,
            started: 0,
        }
    }
    pub fn clear(&mut self) {
        *self = Self::new();
    }
    pub fn poll(&mut self, now: u32) -> Action {
        if self.state != 0 && now.wrapping_sub(self.started) >= COMMAND_TIMEOUT_US {
            self.clear();
            Action::Stop
        } else {
            Action::None
        }
    }
    pub fn byte(&mut self, byte: u8, now: u32) -> Action {
        if self.poll(now) == Action::Stop {
            return Action::Stop;
        }
        // Stop does not wait for a newline, even inside a half-typed duty.
        if matches!(byte, b'o' | b's' | b'!' | 3 | 27) {
            self.clear();
            return Action::Stop;
        }
        let result = match self.state {
            0 => match byte {
                b'\r' | b'\n' => Action::None,
                b'?' => Action::Status,
                b'd' => {
                    self.state = 1;
                    self.started = now;
                    Action::None
                }
                _ => Action::Stop,
            },
            1 => {
                if byte == b'u' {
                    self.state = 2;
                    Action::None
                } else {
                    Action::Stop
                }
            }
            _ => match byte {
                b'0'..=b'9' if self.digits < 3 => {
                    self.value = self.value * 10 + (byte - b'0') as u16;
                    self.digits += 1;
                    if self.value > MAX_DUTY_TENTHS {
                        Action::Stop
                    } else {
                        Action::None
                    }
                }
                b'\r' | b'\n' if self.digits > 0 => {
                    if self.value == 0 {
                        Action::Stop
                    } else {
                        Action::Duty(self.value)
                    }
                }
                _ => Action::Stop,
            },
        };
        if matches!(result, Action::Stop | Action::Duty(_)) {
            self.clear();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command(bytes: &[u8]) -> Action {
        let mut p = Parser::new();
        let mut result = Action::None;
        for &b in bytes {
            result = p.byte(b, 10);
            if result == Action::Stop {
                break;
            }
        }
        result
    }
    #[test]
    fn all_allowed_duties_and_zero_stop() {
        for duty in 1..=MAX_DUTY_TENTHS {
            assert_eq!(
                command(format!("du{duty}\r").as_bytes()),
                Action::Duty(duty)
            );
        }
        assert_eq!(command(b"du0\r"), Action::Stop);
        assert_eq!(
            command(format!("du{}\r", MAX_DUTY_TENTHS).as_bytes()),
            Action::Duty(MAX_DUTY_TENTHS)
        );
    }
    #[test]
    fn no_clamping_overflow_or_partial_acceptance() {
        let overflow = format!("du{}\r", MAX_DUTY_TENTHS + 1);
        assert_eq!(command(overflow.as_bytes()), Action::Stop);
        for bytes in [
            b"du99999999999\r".as_slice(),
            b"du0000\r",
            b"du\r",
            b"d70\r",
            b"du7.0\r",
            b"du-1\r",
            b"du+1\r",
            b"du70x\r",
            b"du?",
            b"x",
        ] {
            assert_eq!(command(bytes), Action::Stop, "{bytes:?}");
        }
        assert_eq!(command(b"du70"), Action::None);
    }
    #[test]
    fn stop_is_immediate_from_every_parser_state() {
        for prefix in [b"".as_slice(), b"d", b"du", b"du70"] {
            for byte in [b'o', b's', b'!', 3, 27] {
                let mut p = Parser::new();
                for &b in prefix {
                    assert_eq!(p.byte(b, 1), Action::None);
                }
                assert_eq!(p.byte(byte, 2), Action::Stop);
                assert_eq!(p.byte(b'?', 3), Action::Status);
            }
        }
    }
    #[test]
    fn partial_timeout_wrap_and_bytes_do_not_extend_it() {
        for start in [0, u32::MAX - 100] {
            let mut p = Parser::new();
            assert_eq!(p.byte(b'd', start), Action::None);
            assert_eq!(p.byte(b'u', start.wrapping_add(200_000)), Action::None);
            assert_eq!(p.poll(start.wrapping_add(249_999)), Action::None);
            assert_eq!(p.byte(b'7', start.wrapping_add(250_000)), Action::Stop);
            assert_eq!(p.poll(start.wrapping_add(500_000)), Action::None);
        }
    }
    #[test]
    fn crlf_multiple_commands_and_session_reset() {
        let mut p = Parser::new();
        let mut actions = Vec::new();
        for &b in b"du70\r\n?du80\n" {
            let a = p.byte(b, 0);
            if a != Action::None {
                actions.push(a);
            }
        }
        assert_eq!(
            actions,
            vec![Action::Duty(70), Action::Status, Action::Duty(80)]
        );
        p.byte(b'd', 0);
        p.clear();
        assert_eq!(p.byte(b'u', 1), Action::Stop);
    }
}
