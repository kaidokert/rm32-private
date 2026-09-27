//! Optional-observation handshake predicates; no acceptance or commutation.
#[inline(always)]
pub const fn defer(phase: u32, line_live: bool) -> bool {
    phase == 4 && !line_live
}

#[inline(always)]
pub const fn resume(powered: bool, phase: u32, line_live: bool, timer_running: bool) -> bool {
    powered && phase == 4 && line_live && !timer_running
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truth_table_only_optional_idle_timer_can_resume() {
        for phase in 0..=5 {
            for powered in [false, true] {
                for live in [false, true] {
                    for running in [false, true] {
                        assert_eq!(defer(phase, live), phase == 4 && !live);
                        assert_eq!(resume(powered, phase, live, running),
                            (powered, phase, live, running) == (true, 4, true, false));
                    }
                }
            }
        }
    }

    // Explicit peripheral/NVIC abstraction, not a hardware proof. OPM clears CEN
    // at expiry, dispatch consumes NVIC pending, acceptance/stop clear it.
    struct Model { phase: u32, live: bool, running: bool, pending: bool, powered: bool, observations: u32 }
    impl Model {
        fn new() -> Self { Self { phase: 4, live: false, running: true, pending: false, powered: true, observations: 0 } }
        fn expire(&mut self) { self.running = false; self.pending = true; }
        fn dispatch(&mut self) {
            if !self.pending { return; }
            self.pending = false;
            if !self.powered || defer(self.phase, self.live) { return; }
            if self.phase == 4 { self.observations += 1; self.running = true; }
        }
        fn wake(&mut self) {
            if resume(self.powered, self.phase, self.live, self.running) { self.pending = true; }
        }
        fn accept(&mut self) { self.pending = false; self.phase = 1; self.running = true; }
        fn stop(&mut self) { self.powered = false; self.pending = false; self.running = false; self.phase = 0; }
    }

    #[test]
    fn expiry_during_comp_parks_then_refusal_wakes_once() {
        let mut m = Model::new(); m.expire(); m.dispatch();
        assert_eq!((m.phase,m.running,m.observations), (4,false,0));
        m.live = true; m.wake(); m.wake(); m.dispatch(); m.wake(); m.dispatch();
        assert_eq!(m.observations,1);
    }

    #[test]
    fn acceptance_and_stop_cancel_pending_wake() {
        for stopped in [false,true] {
            let mut m = Model::new(); m.expire(); m.dispatch(); m.live=true; m.wake();
            if stopped { m.stop(); } else { m.accept(); }
            m.wake(); m.dispatch();
            assert_eq!(m.observations,0);
            assert_eq!(m.phase,if stopped {0} else {1});
        }
    }

    #[test]
    fn expiry_after_resume_before_wake_does_not_duplicate_observation() {
        let mut m=Model::new(); m.live=true; m.wake(); assert!(!m.pending);
        m.expire(); m.dispatch(); m.wake(); m.dispatch(); assert_eq!(m.observations,1);
    }
}
