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
///
/// 4-byte aligned (8 bytes): at its natural 2-byte alignment the 6-byte copy
/// out of the tune tables became a call to the generic `memcpy` (297
/// instructions on thumbv6m) inside the 20 kHz tick; aligned, LLVM copies it
/// with two word loads/stores (binz WCET gate).
#[derive(Clone, Copy)]
#[repr(C, align(4))]
pub struct Note {
    pub prescaler: u16,
    pub step: u8,
    pub ms: u16,
}

/// Tone request ids (SharedState::tone_request). 0 = none.
pub const TONE_BEACON_BASE: u8 = 1; // 1..=5 → beacon tunes 1-5
pub const TONE_ARMED: u8 = 6; // arming confirmation (play_input tune)

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
    pub fn tick<P: crate::constants::Timing>(
        &mut self,
        request: u8,
        motor_running: bool,
    ) -> ToneAction {
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
                    self.ticks_left = n.ms as u32 * P::CONTROL_TICKS_PER_MS as u32;
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
                self.ticks_left = n.ms as u32 * P::CONTROL_TICKS_PER_MS as u32;
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

/// Tone duty, AM32 `sounds.c` `setVolume()`: CCR = beep_volume (0-11) * 3.
pub const fn tone_duty(beep_volume: u8) -> u16 {
    (if beep_volume > 11 { 11 } else { beep_volume }) as u16 * 3
}

/// A tone transition, for a port's counters (`step`'s hook).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToneEvent {
    Start,
    Silence,
}

/// One control tick of the tone stepper applied to the bridge (S7.4):
/// start a note (motor period, pitch divider, AM32 volume duty, the note's
/// winding pair), silence it, or re-assert the duty while a note plays
/// (the control path's idle duty writes would mute it). `base_period` is
/// the motor PWM period the bridge returns to. `bridge` makes the bridge
/// handle, only on a path that writes. `on_event` runs first on a start or
/// silence.
///
/// Drive is the config's `comp_pwm` bit alone: a note only starts while the
/// motor is not running, and old_routine is a running mode, so the core's
/// drive decision (`bridge::drive_complementary`) reduces to it; AM32's
/// tones likewise follow comp_pwm alone.
#[inline(always)]
pub fn step<P, B, M, F>(
    tone: &mut ToneScheduler,
    request: u8,
    running: bool,
    config: &crate::config::EepromConfig,
    base_period: u16,
    bridge: M,
    on_event: F,
) where
    P: crate::constants::Timing,
    B: crate::bridge::Bridge,
    M: FnOnce() -> B,
    F: FnOnce(ToneEvent),
{
    match tone.tick::<P>(request, running) {
        ToneAction::StartNote(n) => {
            on_event(ToneEvent::Start);
            let mut bridge = bridge();
            bridge.set_period(base_period);
            bridge.set_tone_divider(n.prescaler);
            bridge.set_duty_all(tone_duty(config.beep_volume));
            bridge.com_step(n.step, config.use_comp_pwm());
        }
        ToneAction::Silence => {
            on_event(ToneEvent::Silence);
            let mut bridge = bridge();
            bridge.all_off();
            bridge.set_tone_divider(0);
            bridge.set_period(base_period);
        }
        ToneAction::Idle => {
            if tone.active() {
                bridge().set_duty_all(tone_duty(config.beep_volume));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon2_plays_changed_tone_then_silences() {
        // cmd 2 = playChangedTone: two 150 ms notes (AM32 main.c:1211).
        let mut t = ToneScheduler::default();
        assert!(matches!(
            t.tick::<crate::platform::TestPlatform>(2, false),
            ToneAction::StartNote(_)
        ));
        let mut starts = 1;
        let mut done = false;
        for _ in 0..(2 * 150 * 20 + 10) {
            match t.tick::<crate::platform::TestPlatform>(0, false) {
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
        let mut act = t.tick::<crate::platform::TestPlatform>(3, false);
        for _ in 0..(60 * 10 * 20 + 10) {
            if matches!(act, ToneAction::StartNote(_)) {
                starts += 1;
            }
            if matches!(act, ToneAction::Silence) {
                break;
            }
            act = t.tick::<crate::platform::TestPlatform>(0, false);
        }
        assert_eq!(starts, 60);
        assert!(!t.active());
    }

    #[test]
    fn armed_tune_steps_three_notes() {
        let mut t = ToneScheduler::default();
        let mut starts = 0;
        let mut act = t.tick::<crate::platform::TestPlatform>(TONE_ARMED, false);
        for _ in 0..(3 * 100 * 20 + 10) {
            if matches!(act, ToneAction::StartNote(_)) {
                starts += 1;
            }
            if matches!(act, ToneAction::Silence) {
                break;
            }
            act = t.tick::<crate::platform::TestPlatform>(0, false);
        }
        assert_eq!(starts, 3);
        assert!(!t.active());
    }

    #[test]
    fn running_aborts_immediately() {
        let mut t = ToneScheduler::default();
        assert!(matches!(
            t.tick::<crate::platform::TestPlatform>(1, false),
            ToneAction::StartNote(_)
        ));
        assert!(matches!(
            t.tick::<crate::platform::TestPlatform>(0, true),
            ToneAction::Silence
        ));
        assert!(!t.active());
        // And requests while running are ignored.
        assert!(matches!(
            t.tick::<crate::platform::TestPlatform>(3, true),
            ToneAction::Idle
        ));
    }

    /// S7.4 recording bridge: the writes `step` makes, in order.
    #[derive(Default)]
    struct Rec {
        log: [(u8, u16, u16); 8],
        n: usize,
    }
    impl Rec {
        fn push(&mut self, op: u8, a: u16, b: u16) {
            self.log[self.n] = (op, a, b);
            self.n += 1;
        }
        fn take(&mut self) -> ([(u8, u16, u16); 8], usize) {
            let r = (self.log, self.n);
            *self = Self::default();
            r
        }
    }
    impl crate::hal::PhaseOutput for Rec {
        fn com_step(&mut self, step: u8, complementary: bool) {
            self.push(b'c', step as u16, complementary as u16);
        }
        fn all_off(&mut self) {
            self.push(b'o', 0, 0);
        }
        fn full_brake(&mut self) {}
        fn all_pwm(&mut self, _: bool) {}
        fn proportional_brake(&mut self) {}
    }
    impl crate::bridge::Bridge for Rec {
        fn set_duty_all(&mut self, compare: u16) {
            self.push(b'd', compare, 0);
        }
        fn set_phase_duties(&mut self, _: u16, _: u16, _: u16) {}
        fn set_period(&mut self, period: u16) {
            self.push(b'p', period, 0);
        }
        fn latch(&mut self) {}
        fn set_dead_time_override(&mut self, _: u16) {}
        fn set_tone_divider(&mut self, div: u16) {
            self.push(b'v', div, 0);
        }
    }

    /// S7.4: `step` applies AM32's note sequence (period, pitch, volume
    /// duty clamped at 11 * 3, the note's windings with comp_pwm), re-asserts
    /// the duty while the note plays, and silences on running.
    #[test]
    fn step_drives_the_bridge() {
        let mut t = ToneScheduler::default();
        let mut rec = Rec::default();
        let config = crate::config::EepromConfig {
            beep_volume: 20,
            comp_pwm: 1,
            ..Default::default()
        };
        let mut ev = None;
        let mut run =
            |t: &mut ToneScheduler, rec: &mut Rec, req, running, ev: &mut Option<ToneEvent>| {
                step::<crate::platform::TestPlatform, _, _, _>(
                    t,
                    req,
                    running,
                    &config,
                    3332,
                    || &mut *rec,
                    |e| *ev = Some(e),
                )
            };
        run(&mut t, &mut rec, TONE_BEACON_BASE, false, &mut ev);
        let n = DEFAULT_TONE[0];
        let (log, len) = rec.take();
        assert_eq!(ev.take(), Some(ToneEvent::Start));
        assert_eq!(
            &log[..len],
            &[
                (b'p', 3332, 0),
                (b'v', n.prescaler, 0),
                (b'd', 33, 0),
                (b'c', n.step as u16, 1)
            ]
        );
        run(&mut t, &mut rec, 0, false, &mut ev);
        let (log, len) = rec.take();
        assert_eq!((ev, &log[..len]), (None, &[(b'd', 33, 0)][..]));
        run(&mut t, &mut rec, 0, true, &mut ev);
        let (log, len) = rec.take();
        assert_eq!(ev.take(), Some(ToneEvent::Silence));
        assert_eq!(&log[..len], &[(b'o', 0, 0), (b'v', 0, 0), (b'p', 3332, 0)]);
        run(&mut t, &mut rec, 0, false, &mut ev);
        assert_eq!((ev, rec.take().1), (None, 0), "idle: no writes");
        assert_eq!(tone_duty(5), 15);
    }
}
