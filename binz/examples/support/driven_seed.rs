//! Qualify production-ISR acceptances under forced drive using the existing
//! acquisition sequence/cycle checks. No control logic or gate access here.
//! Timestamp units are half-us, same origin for every acceptance in a run.
use crate::flying_acquire::{RuntimeAcquire, Seed};
// Private helpers only receive previously validated step1..6 and gap2..6.
const fn next_step(step: u8) -> u8 {
    if step == 6 { 1 } else { step + 1 }
}
const fn after_gap(step: u8, gap: u16) -> u8 {
    let sum = step as u16 + gap;
    (if sum > 6 { sum - 6 } else { sum }) as u8
}
const _: () = {
    let mut step = 1;
    while step <= 6 {
        assert!(next_step(step) == step % 6 + 1);
        let mut gap = 2;
        while gap <= 6 {
            assert!(after_gap(step, gap) == ((step as u16 - 1 + gap) % 6 + 1) as u8);
            gap += 1;
        }
        step += 1;
    }
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub epoch: u16,
    pub step: u8,
    pub before: u32,
    pub after: u32,
    pub interval: u16,
}
/// Normal forced startup only: commanded speed is an initial estimator, not
/// a measured flying seed or a lock certificate. Caller supplies a real
/// persistence-qualified acceptance; electrical/tracking guards remain live.
pub fn startup_bootstrap(e: Edge, accepts: usize) -> Option<Seed> {
    if accepts < 6
        || e.epoch < 6
        || !(1..=6).contains(&e.step)
        || e.after.wrapping_sub(e.before) > 40
        || !(667..=2000).contains(&e.interval)
    {
        return None;
    }
    Some(Seed {
        step: e.step,
        edge_tick: e.before,
        interval_ticks: 1666,
    })
}
pub struct Qualification {
    acquire: RuntimeAcquire,
    previous: Option<Edge>,
    ready: Option<Seed>,
    fault: u16,
    start: u32,
    reanchor_allowed: bool,
    reanchors: u16,
    anchor_epoch: u16,
    rolling_startup: bool,
    startup_cycle: bool,
    startup_sum: u32,
    startup_estimator: bool,
    startup_count: u8,
    startup_index: u8,
    startup_edges: [u32; 6],
    startup_cycles: [u32; 4],
    #[cfg(feature = "bench-seed-timing-reanchor")]
    timing_restart: bool,
    #[cfg(feature = "bench-seed-timing-reanchor")]
    restart_fault: u16,
}
impl Qualification {
    pub fn new(start: u32) -> Self {
        Self {
            acquire: RuntimeAcquire::new(start),
            previous: None,
            ready: None,
            fault: 0,
            start,
            reanchor_allowed: false,
            reanchors: 0,
            anchor_epoch: 0,
            rolling_startup: false,
            startup_cycle: false,
            startup_sum: 0,
            startup_estimator: false,
            startup_count: 0,
            startup_index: 0,
            startup_edges: [0; 6],
            startup_cycles: [0; 4],
            #[cfg(feature = "bench-seed-timing-reanchor")]
            timing_restart: false,
            #[cfg(feature = "bench-seed-timing-reanchor")]
            restart_fault: 0,
        }
    }
    /// One fresh window after a forward missing-epoch gap, before any seed.
    /// Original acquisition deadline survives; never bridge the missing edge.
    pub fn with_reanchor(start: u32) -> Self {
        let mut q = Self::new(start);
        q.reanchor_allowed = true;
        q
    }
    /// Normal driven startup may discard forward missing-sector windows, not
    /// join their intervals. Caller bounds the complete drive to40ms. A fresh
    /// window still requires twelve ordered intervals and never refreshes a seed.
    pub fn with_startup_window(start: u32) -> Self {
        let mut q = Self::with_reanchor(start);
        q.rolling_startup = true;
        q
    }
    pub fn with_startup_cycle(start: u32) -> Self {
        let mut q = Self::with_startup_window(start);
        q.startup_cycle = true;
        q
    }
    /// Normal startup only. Sector-contiguous measured intervals; individual
    /// spacing/cycle quality is not borrowed from the flying-start envelope.
    pub fn with_startup_estimator(start: u32) -> Self {
        let mut q = Self::with_startup_window(start);
        q.startup_estimator = true;
        q
    }
    /// Candidate only: caller must explicitly select it and retain/report the
    /// discarded fault. One shared restart budget, original acquisition deadline.
    #[cfg(feature = "bench-seed-timing-reanchor")]
    pub fn with_timing_reanchor(start: u32) -> Self {
        let mut q = Self::with_reanchor(start);
        q.timing_restart = true;
        q
    }
    #[cfg(feature = "bench-seed-timing-reanchor")]
    pub fn discarded_fault(&self) -> u16 {
        self.restart_fault
    }
    pub fn reanchors(&self) -> (u16, u16) {
        (self.reanchors, self.anchor_epoch)
    }
    pub fn fault(&self) -> u16 {
        self.fault
    }
    pub fn ready(&self) -> Option<Seed> {
        self.ready
    }
    pub fn intervals(&self) -> u8 {
        if self.startup_estimator {
            self.startup_count
        } else {
            self.acquire.intervals()
        }
    }
    pub fn cycles(&self) -> [u32; 4] {
        if self.startup_estimator {
            self.startup_cycles
        } else {
            self.acquire.cycles
        }
    }
    fn fail(&mut self, code: u16) -> Option<Seed> {
        self.fault = code;
        self.ready = None;
        None
    }
    /// Completed seed is frozen: another event must NEVER refresh its age.
    /// A caller must consume at the acceptance that first returns Some, or
    /// use Seed::handoff's unchanged age/margin check before any transfer.
    pub fn accept(&mut self, e: Edge) -> Option<Seed> {
        if self.fault != 0 {
            return None;
        }
        if self.ready.is_some() {
            return self.ready;
        }
        if !(1..=6).contains(&e.step) || e.after.wrapping_sub(e.before) > 40 {
            return self.fail(1);
        }
        if self.rolling_startup && e.before.wrapping_sub(self.start) > 80000 {
            return self.fail(6);
        }
        // Epoch0 starts partway through a sector inherited from sine startup.
        // Keep its raw IRQ record, but only full commanded sectors may anchor
        // acquisition. This never joins a gap after an admitted first edge.
        if self.previous.is_none() && e.epoch == 0 {
            return None;
        }
        if self.startup_estimator {
            return self.accept_startup(e);
        }
        'history: {
            if let Some(p) = self.previous {
                let gap = e.epoch.wrapping_sub(p.epoch);
                if gap != 1 || e.step != next_step(p.step) {
                    if !self.reanchor_allowed
                        || (!self.rolling_startup && self.reanchors != 0)
                        || !(2..=6).contains(&gap)
                        || e.step != after_gap(p.step, gap)
                        || e.before.wrapping_sub(p.before) == 0
                        || e.before.wrapping_sub(p.before) > 12000
                        || e.before.wrapping_sub(self.start)
                            > (if self.rolling_startup { 80000 } else { 40000 })
                    {
                        return self.fail(2);
                    }
                    self.acquire = RuntimeAcquire::new(if self.rolling_startup {
                        e.before
                    } else {
                        self.start
                    });
                    self.previous = None;
                    self.reanchors += 1;
                    self.anchor_epoch = e.epoch;
                }
            }
            if let Some(p) = self.previous {
                // The preceding block either validated this exact successor or
                // cleared previous for a new anchor. No mutation can bypass it.
                let delta = e.before.wrapping_sub(p.before);
                if delta == 0 || delta > 2000 {
                    if self.rolling_startup && delta > 2000 && delta <= 12000 {
                        let lower = e.before.wrapping_sub(p.after).saturating_sub(2);
                        let upper = e.after.wrapping_sub(p.before).saturating_add(2);
                        if !(lower..=upper).contains(&(e.interval as u32)) {
                            return self.fail(4);
                        }
                        self.acquire = RuntimeAcquire::new(e.before);
                        self.previous = None;
                        self.reanchors += 1;
                        self.anchor_epoch = e.epoch;
                        #[cfg(feature = "bench-seed-timing-reanchor")]
                        {
                            self.restart_fault = 3;
                        }
                        break 'history;
                    }
                    #[cfg(feature = "bench-seed-timing-reanchor")]
                    if self.timing_restart
                        && self.reanchors == 0
                        && delta > 2000
                        && delta <= 12000
                        && e.before.wrapping_sub(self.start) <= 40000
                    {
                        // Corroborate the rejected interval before discarding it.
                        // Do not disguise a stale/corrupt TIM2 reading as startup.
                        let lower = e.before.wrapping_sub(p.after).saturating_sub(2);
                        let upper = e.after.wrapping_sub(p.before).saturating_add(2);
                        if !(lower..=upper).contains(&(e.interval as u32)) {
                            return self.fail(4);
                        }
                        self.acquire.restart_window();
                        self.previous = None;
                        self.reanchors = 1;
                        self.anchor_epoch = e.epoch;
                        self.restart_fault = 3;
                        // Common anchor path; top-level validation already passed.
                        // No recursive re-entry or repeated input/ownership checks.
                        break 'history;
                    }
                    return self.fail(3);
                }
                // TIM2 samples/resets inside the brackets; endpoint quantization
                // is bounded by two half-us ticks, not arbitrary timestamp slack.
                let lower = e.before.wrapping_sub(p.after).saturating_sub(2);
                let upper = e.after.wrapping_sub(p.before).saturating_add(2);
                if !(lower..=upper).contains(&(e.interval as u32)) {
                    return self.fail(4);
                }
            }
        }
        if self.rolling_startup && self.previous.is_none() {
            self.acquire = RuntimeAcquire::new(e.before);
            self.startup_sum = 0;
        }
        let phase = [2, 0, 1, 2, 0, 1][e.step as usize - 1];
        // Earliest bracket time is deliberately conservative for seed age.
        // First prepare-relative TIM2 interval is NEVER used as an interval.
        match self.acquire.edge(phase, e.step & 1 != 0, e.before) {
            Ok(mut seed) => {
                if self.startup_cycle {
                    if let Some(p) = self.previous {
                        self.startup_sum += e.before.wrapping_sub(p.before);
                    }
                    if self.acquire.intervals() == 6 && self.acquire.cycles[0] == 1 {
                        seed = Some(Seed {
                            step: e.step,
                            edge_tick: e.before,
                            interval_ticks: mean_six(self.startup_sum),
                        });
                    }
                }
                self.previous = Some(e);
                self.ready = seed;
                seed
            }
            Err(_) => self.fail(5),
        }
    }
    fn accept_startup(&mut self, e: Edge) -> Option<Seed> {
        if let Some(p) = self.previous {
            let gap = e.epoch.wrapping_sub(p.epoch);
            let delta = e.before.wrapping_sub(p.before);
            if delta == 0 || delta > 12000 {
                return self.fail(3);
            }
            let lower = e.before.wrapping_sub(p.after).saturating_sub(2);
            let upper = e.after.wrapping_sub(p.before).saturating_add(2);
            if !(lower..=upper).contains(&(e.interval as u32)) {
                return self.fail(4);
            }
            if gap != 1 || e.step != next_step(p.step) {
                if !(2..=6).contains(&gap) || e.step != after_gap(p.step, gap) {
                    return self.fail(2);
                }
                self.previous = None;
                self.reanchors += 1;
                self.anchor_epoch = e.epoch;
            } else {
                self.startup_sum += delta;
                self.startup_count += 1;
                if self.startup_count >= 6 {
                    let cycle = e
                        .before
                        .wrapping_sub(self.startup_edges[self.startup_index as usize]);
                    let c = &mut self.startup_cycles;
                    if c[0] == 0 {
                        c[1] = cycle;
                        c[2] = cycle;
                    } else {
                        c[1] = c[1].min(cycle);
                        c[2] = c[2].max(cycle);
                    }
                    c[0] += 1;
                }
                if self.startup_count == 12 {
                    // Preserve the existing valid seed/arm geometry. Only the
                    // average must fit; a late/early pair need not restart it.
                    if (12 * crate::flying_acquire::SEED_MIN_TICKS..=24000)
                        .contains(&self.startup_sum)
                    {
                        let seed = Seed {
                            step: e.step,
                            edge_tick: e.before,
                            interval_ticks: (self.startup_sum * 21_846) >> 18,
                        };
                        self.ready = Some(seed);
                        self.previous = Some(e);
                        return Some(seed);
                    }
                    self.previous = None;
                    self.reanchors += 1;
                    self.anchor_epoch = e.epoch;
                }
            }
        }
        if self.previous.is_none() {
            self.startup_count = 0;
            self.startup_sum = 0;
            self.startup_index = 0;
            self.startup_cycles = [0; 4];
        }
        self.startup_edges[self.startup_index as usize] = e.before;
        self.startup_index = if self.startup_index == 5 {
            0
        } else {
            self.startup_index + 1
        };
        self.previous = Some(e);
        None
    }
}
// Only six validated intervals <=2000 ticks reach this function. Exact over
// that complete domain, no multiply-high, wide product, division or fallback.
fn mean_six(sum: u32) -> u32 {
    (sum * 43_691) >> 18
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_estimator_preserves_late_early_pairs_and_freezes_real_seed() {
        for start in [0, u32::MAX - 5000] {
            let mut q = Qualification::with_startup_estimator(start);
            let mut tick = start.wrapping_add(1000);
            for k in 0..=12u16 {
                let delta = if k & 1 == 1 { 2200 } else { 1132 };
                if k != 0 {
                    tick = tick.wrapping_add(delta);
                }
                let step = (k % 6 + 1) as u8;
                let got = q.accept(Edge {
                    epoch: k + 1,
                    step,
                    before: tick,
                    after: tick.wrapping_add(10),
                    interval: delta as u16,
                });
                assert_eq!(got.is_some(), k == 12);
            }
            let seed = q.ready().unwrap();
            assert_eq!(seed.interval_ticks, 1666);
            assert_eq!(q.intervals(), 12);
            assert_eq!(q.cycles()[..3], [7, 9996, 9996]);
            assert_eq!(
                q.accept(Edge {
                    epoch: 14,
                    step: 2,
                    before: tick.wrapping_add(1666),
                    after: tick.wrapping_add(1676),
                    interval: 1666
                }),
                Some(seed)
            );
        }
    }
    #[test]
    fn startup_estimator_never_averages_missing_epochs_or_corrupt_timer() {
        let edge = |epoch, step, tick, interval| Edge {
            epoch,
            step,
            before: tick,
            after: tick + 10,
            interval,
        };
        let mut q = Qualification::with_startup_estimator(0);
        q.accept(edge(1, 1, 1000, 1666));
        q.accept(edge(2, 2, 2666, 1666));
        q.accept(edge(4, 4, 5998, 3332));
        assert_eq!(q.intervals(), 0);
        assert_eq!(q.reanchors(), (1, 4));
        q.accept(edge(5, 5, 7664, 100));
        assert_eq!(q.fault(), 4);
        let mut q = Qualification::with_startup_estimator(0);
        q.accept(edge(1, 1, 1000, 1666));
        q.accept(edge(2, 2, 14000, 13000));
        assert_eq!(q.fault(), 3);
        let mut q = Qualification::with_startup_estimator(0);
        q.accept(edge(1, 1, 1000, 1666));
        q.accept(edge(2, 2, 81000, 14464));
        assert_eq!(q.fault(), 6);
    }
    #[test]
    fn startup_reciprocal_is_exact_over_entire_bounded_domain() {
        for sum in 0..=24000u32 {
            assert_eq!((sum * 21_846) >> 18, sum / 12);
        }
    }
    #[test]
    fn bootstrap_requires_real_fresh_acceptance_but_not_contiguous_history() {
        let e = Edge {
            epoch: 8,
            step: 3,
            before: 12000,
            after: 12020,
            interval: 1666,
        };
        assert!(startup_bootstrap(e, 5).is_none());
        assert_eq!(
            startup_bootstrap(e, 6),
            Some(Seed {
                step: 3,
                edge_tick: 12000,
                interval_ticks: 1666
            })
        );
        for interval in [0, 666, 2001, 5000] {
            assert!(startup_bootstrap(Edge { interval, ..e }, 6).is_none());
        }
        assert!(startup_bootstrap(Edge { epoch: 5, ..e }, 6).is_none());
        assert!(startup_bootstrap(Edge { after: 12041, ..e }, 6).is_none());
        assert!(startup_bootstrap(Edge { step: 0, ..e }, 6).is_none());
    }
    fn edge(i: u16, start: u32) -> Edge {
        Edge {
            epoch: i + 1,
            step: (i % 6 + 1) as u8,
            before: start.wrapping_add(i as u32 * 1600),
            after: start.wrapping_add(i as u32 * 1600 + 20),
            interval: if i == 0 { 60000 } else { 1600 },
        }
    }
    #[test]
    fn one_complete_cycle_seeds_only_normal_startup_and_freezes_timestamp() {
        let mut q = Qualification::with_startup_cycle(0);
        for i in 0..=6 {
            assert_eq!(q.accept(edge(i, 0)).is_some(), i == 6);
        }
        let seed = q.ready().unwrap();
        assert_eq!(seed.interval_ticks, 1600);
        assert_eq!(q.cycles(), [1, 9600, 9600, 0]);
        assert_eq!(q.intervals(), 6);
        assert_eq!(q.accept(edge(7, 0)), Some(seed));
        assert_eq!(seed.edge_tick, 9600);
        let mut normal = Qualification::new(0);
        for i in 0..=6 {
            assert_eq!(normal.accept(edge(i, 0)), None);
        }
    }
    #[test]
    fn bounded_six_interval_average_is_exact_exhaustively() {
        for sum in 0..=12000 {
            assert_eq!(mean_six(sum), sum / 6);
        }
    }
    #[test]
    fn startup_reanchors_multiple_gaps_without_joining_or_refreshing_seed() {
        let mut q = Qualification::with_startup_window(0);
        for i in [0, 2, 4] {
            let mut e = edge(i, 0);
            e.interval = 3200;
            q.accept(e);
        }
        assert_eq!(q.reanchors(), (2, 5));
        assert_eq!(q.intervals(), 0);
        for i in 5..=16 {
            assert_eq!(q.accept(edge(i, 0)).is_some(), i == 16);
        }
        let seed = q.ready().unwrap();
        assert_eq!(seed.interval_ticks, 1600);
        assert_eq!(q.accept(edge(20, 0)), Some(seed));
        assert_eq!(q.ready().unwrap().edge_tick, 25600);
    }
    #[test]
    fn startup_overall_deadline_duplicate_and_corrupt_gap_still_refuse() {
        let mut q = Qualification::with_startup_window(0);
        q.accept(edge(0, 50000)); // a late anchor gets its own fresh window
        assert_eq!(q.fault(), 0);
        q.accept(edge(1, 80001));
        assert_eq!(q.fault(), 6);
        let mut q = Qualification::with_startup_window(0);
        q.accept(edge(0, 0));
        q.accept(edge(0, 0));
        assert_eq!(q.fault(), 2);
        let mut q = Qualification::with_startup_window(0);
        q.accept(edge(0, 0));
        let mut e = edge(2, 0);
        e.step = 4;
        q.accept(e);
        assert_eq!(q.fault(), 2);
    }
    #[test]
    fn startup_long_transient_is_discarded_only_when_timer_corroborates() {
        let mut q = Qualification::with_startup_window(0);
        q.accept(edge(0, 0));
        let mut anchor = edge(1, 0);
        anchor.before = 2400;
        anchor.after = 2420;
        anchor.interval = 2400;
        q.accept(anchor);
        assert_eq!(q.fault(), 0);
        assert_eq!(q.intervals(), 0);
        for i in 2..=13 {
            let mut e = edge(i, 800);
            e.interval = 1600;
            assert_eq!(q.accept(e).is_some(), i == 13);
        }
        assert_eq!(q.ready().unwrap().interval_ticks, 1600);
        let mut q = Qualification::with_startup_window(0);
        q.accept(edge(0, 0));
        anchor.interval = 1000;
        q.accept(anchor);
        assert_eq!(q.fault(), 4);
    }
    #[test]
    fn thirteen_acceptances_make_twelve_intervals_not_thirteen() {
        for start in [200, u32::MAX - 1000] {
            let mut q = Qualification::new(start);
            for i in 0..13 {
                assert_eq!(q.accept(edge(i, start)).is_some(), i == 12);
            }
            let s = q.ready().unwrap();
            assert_eq!(s.interval_ticks, 1600);
            assert_eq!(s.edge_tick, start.wrapping_add(19200));
            assert_eq!(q.cycles(), [7, 9600, 9600, 0]);
            assert_eq!(q.intervals(), 12);
            assert_eq!(q.accept(edge(13, start)), Some(s));
            assert_eq!(s.handoff(s.edge_tick.wrapping_add(337), 400), None);
            assert_eq!(
                s.handoff(s.edge_tick.wrapping_add(336), 400),
                Some((336, 64))
            );
        }
    }
    #[test]
    fn partial_sector_is_retained_by_caller_but_never_anchors_seed() {
        let mut q = Qualification::new(0);
        let mut partial = edge(0, 0);
        partial.epoch = 0;
        assert_eq!(q.accept(partial), None);
        assert_eq!(q.intervals(), 0);
        // Full epoch1 may have no acceptance; acquisition starts at epoch2.
        for i in 0..13 {
            let mut e = edge(i, 1000);
            e.epoch += 1;
            assert_eq!(q.accept(e).is_some(), i == 12);
        }
        assert_eq!(q.fault(), 0);
        assert_eq!(q.ready().unwrap().edge_tick, 20200);
    }
    #[test]
    fn missing_or_duplicate_epoch_latches_refusal() {
        for i in [0, 2, 6] {
            let mut q = Qualification::new(0);
            q.accept(edge(0, 0));
            q.accept(edge(i, 0));
            assert_eq!(q.fault(), 2);
            for j in 1..14 {
                assert_eq!(q.accept(edge(j, 0)), None);
            }
        }
    }
    #[test]
    fn recorded_low_hysteresis_entry_latches_long_interval() {
        // hystlow_hold64_01 DI85: microsecond endpoints converted to half-us.
        // Epoch0 is partial, epoch2 anchors, epoch4 uses the one allowed restart.
        let rows = [
            [0, 4, 869, 881, 1319],
            [2, 6, 2529, 2542, 3320],
            [4, 2, 4145, 4157, 3230],
            [5, 3, 5264, 5277, 2238],
            [6, 4, 5697, 5710, 866],
            [7, 5, 6747, 6760, 2100],
        ];
        let mut q = Qualification::with_reanchor(628);
        for (i, r) in rows.iter().enumerate() {
            let e = Edge {
                epoch: r[0] as u16,
                step: r[1] as u8,
                before: r[2] * 2,
                after: r[3] * 2,
                interval: r[4] as u16,
            };
            assert_eq!(q.accept(e), None);
            assert_eq!(q.fault(), if i >= 3 { 3 } else { 0 });
        }
        assert_eq!(q.reanchors(), (1, 4));
        assert_eq!(q.ready(), None);
        // The first fatal gap is2238 ticks, not a lack of later IRQ inputs.
        assert_eq!(2 * (5264 - 4145), 2238);
    }
    #[test]
    fn reanchor_requires_twelve_entirely_new_intervals() {
        let mut q = Qualification::with_reanchor(0);
        q.accept(edge(0, 0));
        // Missing epoch2; epoch3 becomes a NEW anchor. Its3200tick gap is not
        // averaged into the fresh twelve intervals, and no timer is reset.
        let mut anchor = edge(2, 0);
        anchor.interval = 3200;
        q.accept(anchor);
        assert_eq!(q.reanchors(), (1, 3));
        assert_eq!(q.intervals(), 0);
        for i in 3..=14 {
            assert_eq!(q.accept(edge(i, 0)).is_some(), i == 14);
        }
        let s = q.ready().unwrap();
        assert_eq!(s.interval_ticks, 1600);
        assert_eq!(s.edge_tick, 22400);
        assert_eq!(q.cycles(), [7, 9600, 9600, 0]);
        assert_eq!(q.accept(edge(20, 0)), Some(s)); // completed seed never refreshed
    }
    #[test]
    fn recorded_failed48_candidate_replays_without_joining_gap() {
        // DI85 from cpu_union_range_reentry48_01. This is offline replay,
        // NOT evidence that the old hardware attempt handed off or recovered.
        let rows: [[u16; 5]; 14] = [
            [1, 2, 1456, 1470, 2437],
            [3, 4, 3069, 3084, 3227],
            [4, 5, 3920, 3934, 1699],
            [5, 6, 4667, 4682, 1495],
            [6, 1, 5575, 5589, 1814],
            [7, 2, 6255, 6269, 1360],
            [8, 3, 7061, 7075, 1612],
            [9, 4, 7877, 7891, 1630],
            [10, 5, 8730, 8744, 1706],
            [11, 6, 9376, 9390, 1290],
            [12, 1, 10208, 10222, 1664],
            [13, 2, 10934, 10948, 1451],
            [14, 3, 11737, 11752, 1606],
            [15, 4, 12465, 12479, 1453],
        ];
        let mut old = Qualification::new(0);
        let mut new = Qualification::with_reanchor(0);
        for (i, r) in rows.iter().enumerate() {
            let e = Edge {
                epoch: r[0],
                step: r[1] as u8,
                before: r[2] as u32 * 2,
                after: r[3] as u32 * 2,
                interval: r[4],
            };
            old.accept(e);
            assert_eq!(new.accept(e).is_some(), i == 13);
        }
        assert_eq!(old.fault(), 2);
        assert_eq!(old.ready(), None);
        assert_eq!(new.ready().unwrap().interval_ticks, 1566);
        assert_eq!(new.ready().unwrap().edge_tick, 24930);
        assert_eq!(new.reanchors(), (1, 3));
    }
    #[test]
    fn reanchor_does_not_allow_duplicates_second_gap_or_deadline_extension() {
        let mut q = Qualification::with_reanchor(0);
        q.accept(edge(0, 0));
        q.accept(edge(0, 0));
        assert_eq!(q.fault(), 2);
        let mut q = Qualification::with_reanchor(0);
        q.accept(edge(0, 0));
        q.accept(edge(2, 0));
        q.accept(edge(4, 0));
        assert_eq!(q.fault(), 2);
        let mut q = Qualification::with_reanchor(0);
        q.accept(edge(0, 30000));
        q.accept(edge(2, 30000));
        for i in 3..=14 {
            q.accept(edge(i, 30000));
        }
        assert!(q.ready().is_none());
        assert_ne!(q.fault(), 0);
        let mut q = Qualification::with_reanchor(0);
        q.accept(edge(0, 0));
        let mut e = edge(2, 0);
        e.step = 4;
        q.accept(e);
        assert_eq!(q.fault(), 2);
    }
    #[test]
    fn bracket_interval_and_time_disagreement_refuse() {
        for fault in 0..4 {
            let mut q = Qualification::new(0);
            q.accept(edge(0, 0));
            let mut e = edge(1, 0);
            match fault {
                0 => e.after = e.before + 41,
                1 => e.interval = 1200,
                2 => {
                    e.before = 0;
                    e.after = 20;
                }
                _ => e.step = 4,
            }
            assert_eq!(q.accept(e), None);
            assert_ne!(q.fault(), 0);
        }
    }
    #[test]
    fn fast_cycle_is_not_rescued_by_valid_individual_gaps() {
        let mut q = Qualification::new(0);
        // Stay below the active full-cycle floor. A fixed1000-tick gap is
        // legitimately inside the335 profile and no longer tests this fault.
        let gap = (super::super::flying_acquire::CYCLE_MIN_TICKS - 1) / 6;
        assert!(gap >= super::super::flying_acquire::INDIVIDUAL_MIN_TICKS);
        for i in 0..13 {
            let mut e = edge(i, 0);
            e.before = i as u32 * gap;
            e.after = e.before + 20;
            e.interval = gap as u16;
            q.accept(e);
        }
        assert_ne!(q.fault(), 0);
        assert_eq!(q.ready(), None);
    }
}
