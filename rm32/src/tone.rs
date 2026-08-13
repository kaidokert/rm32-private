//! Non-blocking tone scheduler.
//!
//! AM32 plays beacon/arming tunes with blocking delay loops from main
//! (its main owns the hardware). rm32's HAL lives in ISR state, so
//! tones are stepped from the 20 kHz tick instead: a request id arrives
//! via `SharedState::tone_request` (from the DSHOT command ISR or from
//! main's arming path), and the scheduler applies one note's PWM/phase
//! setup when the note starts, counts ticks, then silences. Constant
//! per-tick cost (one state check; note transitions are rare and
//! cheap), and the tone aborts instantly if the motor starts running.
//!
//! Note primitive parity: `Sounds::play_note` = set ARR + prescaler +
//! duty(volume) + com_step, hold, then `silence` = all_off +
//! prescaler 0 + ARR restore.

/// One tone note: TIM1 prescaler (pitch), commutation step (which
/// windings hum), duration in milliseconds.
#[derive(Clone, Copy)]
pub struct Note {
    pub prescaler: u16,
    pub step: u8,
    pub ms: u16,
}

/// Tone request ids (SharedState::tone_request). 0 = none.
pub const TONE_BEACON_BASE: u8 = 1; // 1..=5 → beacon tunes 1-5
pub const TONE_ARMED: u8 = 6; // arming confirmation (play_input tune)

const TICKS_PER_MS: u32 = 20; // 20 kHz tick

/// What the scheduler wants applied to the hardware this tick.
pub enum ToneAction {
    /// Nothing to do (no tone active).
    Idle,
    /// Start (or continue into) this note: apply ARR+prescaler+duty+step.
    StartNote(Note),
    /// Tone finished or aborted: silence (all_off + prescaler 0 + ARR).
    Silence,
}

#[derive(Default)]
pub struct ToneScheduler {
    seq_id: u8,
    note_idx: u8,
    ticks_left: u32,
    active: bool,
}

/// AM32 beacon tune tables — verbatim from main.c:1204's dispatch:
/// cmd 1/5 → playDefaultTone, cmd 2 → playChangedTone, cmd 3 →
/// playBeaconTune3 (60-note descending sweep), cmd 4 → playInputTune2.
const DEFAULT_TONE: [Note; 2] = [
    Note {
        prescaler: 50,
        step: 2,
        ms: 150,
    },
    Note {
        prescaler: 30,
        step: 2,
        ms: 150,
    },
];
const CHANGED_TONE: [Note; 2] = [
    Note {
        prescaler: 40,
        step: 2,
        ms: 150,
    },
    Note {
        prescaler: 80,
        step: 2,
        ms: 150,
    },
];
const INPUT_TUNE2: [Note; 3] = [
    Note {
        prescaler: 60,
        step: 1,
        ms: 75,
    },
    Note {
        prescaler: 80,
        step: 1,
        ms: 75,
    },
    Note {
        prescaler: 90,
        step: 1,
        ms: 75,
    },
];

/// playBeaconTune3: `for i in (1..=119).rev().step_by(2)`: comStep(i/20),
/// prescaler 10 + i/2, 10 ms each (sounds.c:261). AM32 passes comStep(0)
/// for the last few notes — clamped to 1 here (step 0 is undefined in
/// rm32's phase driver).
fn beacon3_note(idx: u8) -> Option<Note> {
    if idx >= 60 {
        return None;
    }
    let i = 119 - 2 * idx as u16;
    Some(Note {
        prescaler: 10 + i / 2,
        step: ((i / 20) as u8).max(1),
        ms: 10,
    })
}

/// Arming tune = Sounds::play_input (3 descending notes, 100 ms each).
const ARMED_TUNE: [Note; 3] = [
    Note {
        prescaler: 80,
        step: 3,
        ms: 100,
    },
    Note {
        prescaler: 70,
        step: 3,
        ms: 100,
    },
    Note {
        prescaler: 40,
        step: 3,
        ms: 100,
    },
];

impl ToneScheduler {
    fn note_for(&self) -> Option<Note> {
        match self.seq_id {
            1 | 5 => DEFAULT_TONE.get(self.note_idx as usize).copied(),
            2 => CHANGED_TONE.get(self.note_idx as usize).copied(),
            3 => beacon3_note(self.note_idx),
            4 => INPUT_TUNE2.get(self.note_idx as usize).copied(),
            TONE_ARMED => ARMED_TUNE.get(self.note_idx as usize).copied(),
            _ => None,
        }
    }

    /// Step one 20 kHz tick. `request`: pending tone id (consumed on
    /// accept). `motor_running`: aborts any active tone immediately.
    pub fn tick(&mut self, request: u8, motor_running: bool) -> ToneAction {
        if motor_running {
            if self.active {
                self.active = false;
                return ToneAction::Silence;
            }
            return ToneAction::Idle;
        }
        if !self.active {
            if request == 0 {
                return ToneAction::Idle;
            }
            self.seq_id = request;
            self.note_idx = 0;
            self.active = true;
            return match self.note_for() {
                Some(n) => {
                    self.ticks_left = n.ms as u32 * TICKS_PER_MS;
                    ToneAction::StartNote(n)
                }
                None => {
                    self.active = false;
                    ToneAction::Idle
                }
            };
        }
        // Active: count down the current note.
        if self.ticks_left > 1 {
            self.ticks_left -= 1;
            return ToneAction::Idle;
        }
        self.note_idx += 1;
        match self.note_for() {
            Some(n) => {
                self.ticks_left = n.ms as u32 * TICKS_PER_MS;
                ToneAction::StartNote(n)
            }
            None => {
                self.active = false;
                ToneAction::Silence
            }
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon2_plays_changed_tone_then_silences() {
        // cmd 2 = playChangedTone: two 150 ms notes (AM32 main.c:1211).
        let mut t = ToneScheduler::default();
        assert!(matches!(t.tick(2, false), ToneAction::StartNote(_)));
        let mut starts = 1;
        let mut done = false;
        for _ in 0..(2 * 150 * 20 + 10) {
            match t.tick(0, false) {
                ToneAction::StartNote(_) => starts += 1,
                ToneAction::Silence => {
                    done = true;
                    break;
                }
                ToneAction::Idle => {}
            }
        }
        assert!(done);
        assert_eq!(starts, 2);
        assert!(!t.active());
    }

    #[test]
    fn beacon3_sweeps_sixty_notes() {
        let mut t = ToneScheduler::default();
        let mut starts = 0;
        let mut act = t.tick(3, false);
        for _ in 0..(60 * 10 * 20 + 10) {
            if matches!(act, ToneAction::StartNote(_)) {
                starts += 1;
            }
            if matches!(act, ToneAction::Silence) {
                break;
            }
            act = t.tick(0, false);
        }
        assert_eq!(starts, 60);
        assert!(!t.active());
    }

    #[test]
    fn armed_tune_steps_three_notes() {
        let mut t = ToneScheduler::default();
        let mut starts = 0;
        let mut act = t.tick(TONE_ARMED, false);
        for _ in 0..(3 * 100 * 20 + 10) {
            if matches!(act, ToneAction::StartNote(_)) {
                starts += 1;
            }
            if matches!(act, ToneAction::Silence) {
                break;
            }
            act = t.tick(0, false);
        }
        assert_eq!(starts, 3);
        assert!(!t.active());
    }

    #[test]
    fn running_aborts_immediately() {
        let mut t = ToneScheduler::default();
        assert!(matches!(t.tick(1, false), ToneAction::StartNote(_)));
        assert!(matches!(t.tick(0, true), ToneAction::Silence));
        assert!(!t.active());
        // And requests while running are ignored.
        assert!(matches!(t.tick(3, true), ToneAction::Idle));
    }
}
