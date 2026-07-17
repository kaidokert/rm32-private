//! Bench arm/kill/CL mode state machine — the transitions that were
//! scattered across the `r`/`q`/`w`/`y`/`m` keys. Every invariant
//! here has had an incident: estimator-reset-on-arm (poisoned-144
//! deadlock), snap-on-arm (4/4 failed engages), EXTI-mask-when-not-
//! driving (650 k events/s storm), no zombie CL flags after a kill
//! (the OC zombie-status ladder), and the stale-CL-arm-across-mode-
//! toggle trap (roadmap B4 — `m` is now gated on `cl_armed` too).
//!
//! Seam: [`ModeState`] atomic refs + the main loop's [`Mirror`]
//! locals in, [`Actions`] out — the firmware performs the peripheral
//! calls and prints. ISR-side kills do NOT come through here (they
//! run in interrupt context via `guards::apply_isr_kill`); main's
//! trip-report blocks only sync `Mirror::output_enabled`.

use portable_atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, Ordering};

pub struct ModeState<'a> {
    pub motor_enabled: &'a AtomicBool,
    pub cl_active: &'a AtomicBool,
    pub cl_armed: &'a AtomicBool,
    pub cl_reacq: &'a AtomicBool,
    pub cl_noz_run: &'a AtomicU8,
    // Estimator statics (reset on CL arm).
    pub interval_us: &'a AtomicU32,
    /// R2 stiff-average accumulator (reset with the estimator on arm).
    pub avg_interval_acc: &'a AtomicU32,
    pub last_qzc_us: &'a AtomicU32,
    pub windows_since_qzc: &'a AtomicU8,
    // Throttle snap.
    pub amplitude_pct: &'a AtomicU8,
    pub amp_target_pct: &'a AtomicU8,
    // Sag baseline capture at arm.
    pub vbat_baseline_raw: &'a AtomicU16,
    pub vbat_live: &'a AtomicU16,
}

/// Main-loop mirror of the drive state (locals in the firmware; a
/// plain struct here so the property tests can drive sequences).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mirror {
    pub output_enabled: bool,
    pub six_step: bool,
    pub hz: u32,
    pub amp_pct: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// `r` / `q`: arm (or retune) the open-loop drive.
    Arm { hz: u32, amp_pct: u16 },
    /// `w`: unconditional kill.
    Kill,
    /// `y`: engage-arm the closed loop, or kill it if active.
    ClToggle,
    /// `m`: sine ↔ six-step.
    ModeToggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    None,
    Armed,
    Off,
    /// "CL active - 'y' first" (or armed, for the `m` gate).
    ClBlocked,
    ClOffKilled,
    ClArmed,
    ClNeedsDrive,
    ModeToggled,
}

/// Peripheral actions for the caller, in application order:
/// `arm_output` before `exti` (arm), `all_off` before `exti` (kill).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actions {
    pub arm_output: bool,
    pub all_off: bool,
    pub exti: Option<bool>,
    pub msg: Msg,
}

impl Actions {
    const fn none(msg: Msg) -> Self {
        Self {
            arm_output: false,
            all_off: false,
            exti: None,
            msg,
        }
    }
}

pub fn step(ms: &ModeState<'_>, mir: &mut Mirror, cmd: Cmd) -> Actions {
    match cmd {
        Cmd::Arm { hz, amp_pct } => {
            if ms.cl_active.load(Ordering::Relaxed) {
                return Actions::none(Msg::ClBlocked);
            }
            // Arm-time vbat baseline for the −10 % sag kill (captured
            // unloaded, before the drive engages).
            ms.vbat_baseline_raw
                .store(ms.vbat_live.load(Ordering::Relaxed), Ordering::Relaxed);
            mir.six_step = true;
            mir.hz = hz;
            mir.amp_pct = amp_pct;
            // SNAP applied = target on arm: the slew is for changes
            // while RUNNING (the 4/4 failed-engages incident).
            ms.amplitude_pct.store(amp_pct as u8, Ordering::Relaxed);
            ms.amp_target_pct.store(amp_pct as u8, Ordering::Relaxed);
            if !mir.output_enabled {
                mir.output_enabled = true;
                ms.motor_enabled.store(true, Ordering::Relaxed);
                Actions {
                    arm_output: true,
                    all_off: false,
                    // Re-arm BEMF EXTI only now that the FETs drive.
                    exti: Some(true),
                    msg: Msg::Armed,
                }
            } else {
                Actions::none(Msg::None)
            }
        }
        Cmd::Kill => {
            // UNCONDITIONAL (roadmap F1): actions independent of the
            // mirror; all idempotent.
            ms.cl_active.store(false, Ordering::Relaxed);
            ms.cl_armed.store(false, Ordering::Relaxed);
            ms.motor_enabled.store(false, Ordering::Relaxed);
            let was_on = mir.output_enabled;
            mir.output_enabled = false;
            Actions {
                arm_output: false,
                all_off: true,
                exti: Some(false),
                msg: if was_on { Msg::Off } else { Msg::None },
            }
        }
        Cmd::ClToggle => {
            if ms.cl_active.load(Ordering::Relaxed) {
                ms.cl_active.store(false, Ordering::Relaxed);
                ms.cl_armed.store(false, Ordering::Relaxed);
                ms.motor_enabled.store(false, Ordering::Relaxed);
                mir.output_enabled = false;
                Actions {
                    arm_output: false,
                    all_off: true,
                    exti: Some(false),
                    msg: Msg::ClOffKilled,
                }
            } else if mir.output_enabled && mir.six_step {
                // Reset the estimator before arming: a stale interval
                // (e.g. 144 µs left by a runaway) is otherwise
                // UNRECOVERABLE — the runaway floor kills every engage
                // while the symmetric bound rejects every honest
                // open-loop sample. Engagement waits for a fresh
                // estimate, so this re-seeds within a few windows.
                ms.interval_us.store(0, Ordering::Relaxed);
                ms.avg_interval_acc.store(0, Ordering::Relaxed);
                ms.last_qzc_us.store(u32::MAX, Ordering::Relaxed);
                ms.windows_since_qzc.store(0, Ordering::Relaxed);
                ms.cl_reacq.store(false, Ordering::Relaxed);
                ms.cl_noz_run.store(0, Ordering::Relaxed);
                ms.cl_armed.store(true, Ordering::Relaxed);
                Actions::none(Msg::ClArmed)
            } else {
                Actions::none(Msg::ClNeedsDrive)
            }
        }
        Cmd::ModeToggle => {
            // Gated on ARMED as well as ACTIVE (roadmap B4): toggling
            // to sine with a pending CL arm zeroes the float mask, no
            // windows ever close, and the stale arm engages
            // unexpectedly minutes later when toggled back.
            if ms.cl_active.load(Ordering::Relaxed) || ms.cl_armed.load(Ordering::Relaxed) {
                Actions::none(Msg::ClBlocked)
            } else {
                mir.six_step = !mir.six_step;
                Actions::none(Msg::ModeToggled)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rig {
        motor_enabled: AtomicBool,
        cl_active: AtomicBool,
        cl_armed: AtomicBool,
        cl_reacq: AtomicBool,
        cl_noz_run: AtomicU8,
        interval_us: AtomicU32,
        avg_interval_acc: AtomicU32,
        last_qzc_us: AtomicU32,
        windows_since_qzc: AtomicU8,
        amplitude_pct: AtomicU8,
        amp_target_pct: AtomicU8,
        vbat_baseline_raw: AtomicU16,
        vbat_live: AtomicU16,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                motor_enabled: AtomicBool::new(false),
                cl_active: AtomicBool::new(false),
                cl_armed: AtomicBool::new(false),
                cl_reacq: AtomicBool::new(false),
                cl_noz_run: AtomicU8::new(0),
                interval_us: AtomicU32::new(0),
                avg_interval_acc: AtomicU32::new(0),
                last_qzc_us: AtomicU32::new(u32::MAX),
                windows_since_qzc: AtomicU8::new(0),
                amplitude_pct: AtomicU8::new(15),
                amp_target_pct: AtomicU8::new(15),
                vbat_baseline_raw: AtomicU16::new(0),
                vbat_live: AtomicU16::new(1080),
            }
        }

        fn state(&self) -> ModeState<'_> {
            ModeState {
                motor_enabled: &self.motor_enabled,
                cl_active: &self.cl_active,
                cl_armed: &self.cl_armed,
                cl_reacq: &self.cl_reacq,
                cl_noz_run: &self.cl_noz_run,
                interval_us: &self.interval_us,
                avg_interval_acc: &self.avg_interval_acc,
                last_qzc_us: &self.last_qzc_us,
                windows_since_qzc: &self.windows_since_qzc,
                amplitude_pct: &self.amplitude_pct,
                amp_target_pct: &self.amp_target_pct,
                vbat_baseline_raw: &self.vbat_baseline_raw,
                vbat_live: &self.vbat_live,
            }
        }
    }

    fn idle_mirror() -> Mirror {
        Mirror {
            output_enabled: false,
            six_step: true,
            hz: 0,
            amp_pct: 15,
        }
    }

    const ARM: Cmd = Cmd::Arm {
        hz: 60,
        amp_pct: 15,
    };

    #[test]
    fn arm_snaps_baselines_and_enables() {
        let r = Rig::new();
        let mut mir = idle_mirror();
        r.amplitude_pct.store(50, Ordering::Relaxed); // stale slew
        r.amp_target_pct.store(50, Ordering::Relaxed);
        let a = step(&r.state(), &mut mir, ARM);
        assert!(a.arm_output);
        assert_eq!(a.exti, Some(true));
        assert_eq!(a.msg, Msg::Armed);
        assert!(mir.output_enabled);
        assert!(r.motor_enabled.load(Ordering::Relaxed));
        // Snap-on-arm regression: applied AND target set together.
        assert_eq!(r.amplitude_pct.load(Ordering::Relaxed), 15);
        assert_eq!(r.amp_target_pct.load(Ordering::Relaxed), 15);
        // Sag baseline captured from the live sample.
        assert_eq!(r.vbat_baseline_raw.load(Ordering::Relaxed), 1080);
    }

    #[test]
    fn arm_while_running_retunes_without_rearming() {
        let r = Rig::new();
        let mut mir = idle_mirror();
        step(&r.state(), &mut mir, ARM);
        let a = step(
            &r.state(),
            &mut mir,
            Cmd::Arm {
                hz: 50,
                amp_pct: 15,
            },
        );
        assert!(!a.arm_output, "already armed: no double arm_output");
        assert_eq!(a.exti, None, "no EXTI churn while running");
        assert_eq!(mir.hz, 50);
    }

    #[test]
    fn kill_is_unconditional_and_idempotent() {
        let r = Rig::new();
        let mut mir = idle_mirror();
        // F1 regression: even with a desynced mirror (false while the
        // atomics say driving), Kill must emit the hardware actions.
        r.motor_enabled.store(true, Ordering::Relaxed);
        r.cl_active.store(true, Ordering::Relaxed);
        r.cl_armed.store(true, Ordering::Relaxed);
        let a = step(&r.state(), &mut mir, Cmd::Kill);
        assert!(a.all_off);
        assert_eq!(a.exti, Some(false));
        assert!(!r.motor_enabled.load(Ordering::Relaxed));
        assert!(!r.cl_active.load(Ordering::Relaxed), "zombie CL_ACTIVE");
        assert!(!r.cl_armed.load(Ordering::Relaxed), "zombie CL_ARMED");
        // Second kill: same actions, no print.
        let a = step(&r.state(), &mut mir, Cmd::Kill);
        assert!(a.all_off);
        assert_eq!(a.msg, Msg::None);
    }

    #[test]
    fn regression_cl_arm_resets_poisoned_estimator() {
        let r = Rig::new();
        let mut mir = idle_mirror();
        step(&r.state(), &mut mir, ARM);
        // Poison from a previous runaway:
        r.interval_us.store(144, Ordering::Relaxed);
        r.last_qzc_us.store(12345, Ordering::Relaxed);
        r.windows_since_qzc.store(7, Ordering::Relaxed);
        r.cl_reacq.store(true, Ordering::Relaxed);
        r.cl_noz_run.store(2, Ordering::Relaxed);
        let a = step(&r.state(), &mut mir, Cmd::ClToggle);
        assert_eq!(a.msg, Msg::ClArmed);
        assert!(r.cl_armed.load(Ordering::Relaxed));
        assert_eq!(r.interval_us.load(Ordering::Relaxed), 0);
        assert_eq!(r.last_qzc_us.load(Ordering::Relaxed), u32::MAX);
        assert_eq!(r.windows_since_qzc.load(Ordering::Relaxed), 0);
        assert!(!r.cl_reacq.load(Ordering::Relaxed));
        assert_eq!(r.cl_noz_run.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn cl_toggle_gating_matrix() {
        // Dead drive: refused.
        let r = Rig::new();
        let mut mir = idle_mirror();
        assert_eq!(
            step(&r.state(), &mut mir, Cmd::ClToggle).msg,
            Msg::ClNeedsDrive
        );
        // Sine drive: refused.
        step(&r.state(), &mut mir, ARM);
        mir.six_step = false;
        assert_eq!(
            step(&r.state(), &mut mir, Cmd::ClToggle).msg,
            Msg::ClNeedsDrive
        );
        // Active: toggle kills.
        mir.six_step = true;
        r.cl_active.store(true, Ordering::Relaxed);
        let a = step(&r.state(), &mut mir, Cmd::ClToggle);
        assert_eq!(a.msg, Msg::ClOffKilled);
        assert!(a.all_off);
        assert!(!mir.output_enabled);
    }

    #[test]
    fn regression_b4_mode_toggle_blocked_while_cl_armed() {
        // The stale-arm trap: r → y (armed) → m to sine would strand
        // a pending CL arm that engages unexpectedly minutes later.
        let r = Rig::new();
        let mut mir = idle_mirror();
        step(&r.state(), &mut mir, ARM);
        step(&r.state(), &mut mir, Cmd::ClToggle); // armed
        let a = step(&r.state(), &mut mir, Cmd::ModeToggle);
        assert_eq!(a.msg, Msg::ClBlocked);
        assert!(mir.six_step, "mode unchanged");
        // After a kill the toggle works again.
        step(&r.state(), &mut mir, Cmd::Kill);
        assert_eq!(
            step(&r.state(), &mut mir, Cmd::ModeToggle).msg,
            Msg::ModeToggled
        );
        assert!(!mir.six_step);
    }

    #[test]
    fn arm_blocked_while_cl_active() {
        let r = Rig::new();
        let mut mir = idle_mirror();
        r.cl_active.store(true, Ordering::Relaxed);
        let a = step(&r.state(), &mut mir, ARM);
        assert_eq!(a.msg, Msg::ClBlocked);
        assert!(!a.arm_output);
        assert!(!mir.output_enabled);
    }

    #[test]
    fn property_mirror_never_diverges_from_motor_enabled() {
        // Any command sequence must keep the main-loop mirror and the
        // MOTOR_ENABLED atomic in lockstep (the class behind the
        // swallowed-arm and defanged-kill bugs).
        let r = Rig::new();
        let mut mir = idle_mirror();
        let cmds = [
            ARM,
            Cmd::ClToggle,
            Cmd::ModeToggle,
            Cmd::Kill,
            Cmd::ModeToggle,
            Cmd::ClToggle,
            ARM,
            ARM,
            Cmd::ClToggle,
            Cmd::Kill,
            Cmd::Kill,
            ARM,
        ];
        for (i, c) in cmds.iter().enumerate() {
            // Simulate the ISR engaging CL between commands when armed.
            if r.cl_armed.load(Ordering::Relaxed) && i % 2 == 0 {
                r.cl_armed.store(false, Ordering::Relaxed);
                r.cl_active.store(true, Ordering::Relaxed);
            }
            step(&r.state(), &mut mir, *c);
            assert_eq!(
                mir.output_enabled,
                r.motor_enabled.load(Ordering::Relaxed),
                "diverged after cmd {i}: {c:?}"
            );
        }
    }

    #[test]
    fn exti_only_enabled_when_transitioning_to_driving() {
        // The 650 k events/s storm rule: Some(true) must ONLY appear
        // on a dead→driving transition.
        let r = Rig::new();
        let mut mir = idle_mirror();
        for cmd in [Cmd::ClToggle, Cmd::ModeToggle, Cmd::Kill] {
            let a = step(&r.state(), &mut mir, cmd);
            assert_ne!(a.exti, Some(true), "{cmd:?} enabled EXTI while dead");
        }
        assert_eq!(step(&r.state(), &mut mir, ARM).exti, Some(true));
        // Re-arm while running: no EXTI churn.
        assert_eq!(step(&r.state(), &mut mir, ARM).exti, None);
    }
}
