//! Register-generation model. Hardware latch behavior still needs pad evidence.
#[derive(Clone, Copy)]
struct Timer {
    active: [u16; 3], shadow: [u16; 3], udis: bool, ccpc: bool,
    moe: bool, enabled: bool, stopped: bool,
}
impl Timer {
    fn update(&mut self) { if !self.udis { self.active = self.shadow; } }
    fn stop(&mut self) {
        self.stopped = true;
        self.moe = false;
        self.shadow = [0; 3];
        self.enabled = false;
    }
}

#[test]
fn inherited_startup_compares_remain_below_campaign_cap_after_carrier_change() {
    use crate::run::policy::{CATCH_DUTY_TENTHS, DRIVEN_DUTY_TENTHS, SIXSTEP_DUTY_CAP};
    use crate::duty::{STARTUP_TICKS, RUN_PERIOD_TICKS};
    let sine_max = crate::sine::amplitude(STARTUP_TICKS - 1, u32::from(CATCH_DUTY_TENTHS));
    let driven = STARTUP_TICKS * u32::from(DRIVEN_DUTY_TENTHS) / 1000;
    assert_eq!((sine_max, driven), (396, 390));
    assert_eq!(RUN_PERIOD_TICKS, 1333);
    assert!(sine_max.max(driven) * 1000 < RUN_PERIOD_TICKS * u32::from(SIXSTEP_DUTY_CAP));
    assert_eq!(RUN_PERIOD_TICKS * u32::from(SIXSTEP_DUTY_CAP) / 1000, 1066);
}

#[test]
fn unequal_entry_shadows_stay_old_then_become_one_complete_new_generation() {
    // Handover may inherit unequal active values from its immediate writer.
    // The transaction must not pretend to repair those until native UEV.
    for old in [[384, 133, 133], [666, 666, 679], [0, 1, 2]] {
        let mut t = Timer { active: old, shadow: old, udis: false, ccpc: false,
            moe: true, enabled: true, stopped: false };
        t.udis = true;
        t.ccpc = true;
        for i in 0..3 {
            t.shadow[i] = 733;
            t.update();
            assert_eq!(t.active, old);
        }
        t.ccpc = false;
        t.udis = false;
        assert_eq!(t.active, old);
        t.update();
        assert_eq!(t.active, [733; 3]);
    }
}

#[test]
fn pending_guard_wins_after_atomic_transaction_and_native_updates_never_mix_ccrs() {
    for old in [0, 384, 666, 1066] {
        for new in [0, 100, 733, 1066] {
            // Every combination of native overflows at the six staging boundaries.
            for updates in 0..64 {
                for stop_at in 0..=7 {
                    let mut t = Timer { active: [old; 3], shadow: [old; 3],
                        udis: false, ccpc: false, moe: true, enabled: true, stopped: false };
                    if stop_at == 0 { t.stop(); }
                    // This mirrors the production ownership check INSIDE PRIMASK.
                    if !t.stopped {
                        t.udis = true;
                        t.ccpc = true;
                        for op in 0..6 {
                            // CCMR enables every PE before any CCR write; COMG at op4.
                            if (1..=3).contains(&op) { t.shadow[op - 1] = new; }
                            if updates & (1 << op) != 0 { t.update(); }
                            assert_eq!(t.active, [old; 3], "UDIS holds one complete generation");
                        }
                        t.ccpc = false;
                        t.udis = false;
                        t.update();
                        assert_eq!(t.active, [new; 3]);
                        // Any guard request while masked is serviced after the restore.
                        if stop_at < 7 { t.stop(); }
                    }
                    assert!(!t.ccpc && !t.udis);
                    if stop_at < 7 {
                        assert!(!t.moe && !t.enabled && t.stopped);
                        assert_eq!(t.shadow, [0; 3], "no resumed writer after safing");
                    }
                }
            }
        }
    }
}
