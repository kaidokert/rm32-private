//! Bridge-disabled acquisition from independently qualified comparator edges.
//! No gate/timer access and no replacement for the reference BEMF controller.
//! Inputs use reference-HAL polarity, logical A/B/C=0/1/2, half-us ticks.
//! Initial static levels MUST NOT be passed as edges. Caller owns mux settling,
//! sampling continuity and edge persistence; this checks sequence and timing.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    InvalidPhase,
    WrongOrder,
    TooFast,
    TooSlow,
    Expired,
    CycleTooFast,
    CycleTooSlow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seed {
    pub step: u8,
    pub edge_tick: u32,
    pub interval_ticks: u32,
}
/// Proof that every one of twelve consecutive comparator reads matched.
/// The hardware caller owns mux selection and read timing; no analog claim.
#[derive(Debug, PartialEq, Eq)]
pub struct PersistentLevel {
    level: bool,
}
#[inline(always)]
pub fn persistent_level(level: bool, mut read: impl FnMut() -> bool) -> Option<PersistentLevel> {
    for _ in 0..12 {
        if read() != level {
            return None;
        }
    }
    Some(PersistentLevel { level })
}
pub const SEED_MIN_TICKS: u32 = if cfg!(feature = "bench-seed500") {
    667
} else if cfg!(feature = "bench-seed450") {
    741
} else if cfg!(feature = "bench-seed400") {
    834
} else if cfg!(feature = "bench-range350") {
    952
} else if cfg!(feature = "bench-range345") {
    966
} else if cfg!(feature = "bench-range340") {
    980
} else if cfg!(feature = "bench-range335") {
    995
} else if cfg!(feature = "bench-range330") {
    1010
} else if cfg!(feature = "bench-range320") {
    1041
} else if cfg!(feature = "bench-range310") {
    1075
} else if cfg!(feature = "bench-range300") {
    1111
} else {
    1333
};
pub const CYCLE_MIN_TICKS: u32 = if cfg!(feature = "bench-seed500") {
    4000
} else if cfg!(feature = "bench-seed450") {
    4445
} else if cfg!(feature = "bench-seed400") {
    5000
} else if cfg!(feature = "bench-range350") {
    5716
} else if cfg!(feature = "bench-range345") {
    5798
} else if cfg!(feature = "bench-range340") {
    5884
} else if cfg!(feature = "bench-range335") {
    5972
} else if cfg!(feature = "bench-range330") {
    6062
} else if cfg!(feature = "bench-range320") {
    6250
} else if cfg!(feature = "bench-range310") {
    6452
} else if cfg!(feature = "bench-range300") {
    6666
} else {
    8000
};
pub const INDIVIDUAL_MIN_TICKS: u32 = if cfg!(feature = "bench-range350") {
    476
} else if cfg!(feature = "bench-range345") {
    482
} else if cfg!(feature = "bench-range340") {
    490
} else if cfg!(feature = "bench-range335") {
    496
} else if cfg!(feature = "bench-range330") {
    504
} else if cfg!(feature = "bench-range320") {
    520
} else if cfg!(feature = "bench-range310") {
    536
} else if cfg!(feature = "bench-range300") {
    554
} else {
    666
};
pub type RuntimeAcquire = Acquisition<CYCLE_MIN_TICKS, INDIVIDUAL_MIN_TICKS>;
/// Exact for twelve accepted intervals <=2000 ticks each. Preserve ordinary
/// division outside that domain rather than silently assuming a truncated sum.
#[cfg(feature = "bench-seed-div12")]
#[inline(always)]
fn mean_twelve(sum: u32) -> u32 {
    const DIVISOR: u32 = 12;
    const MAX_SUM: u32 = DIVISOR * 2000;
    const SHIFT: u32 = 18;
    const SCALE: u32 = 1 << SHIFT;
    const RECIPROCAL: u32 = (SCALE + DIVISOR - 1) / DIVISOR;
    const _: () = {
        assert!(RECIPROCAL == 21_846);
        // Reciprocal error stays below one remainder unit; product cannot wrap.
        assert!(MAX_SUM * (RECIPROCAL * DIVISOR - SCALE) < SCALE);
        assert!(MAX_SUM <= u32::MAX / RECIPROCAL);
    };
    if sum <= MAX_SUM {
        (sum * RECIPROCAL) >> SHIFT
    } else {
        mean_twelve_fallback(sum)
    }
}
// LLVM speculated plain sum/12 ahead of the bound in E375. The opaque input
// barrier is confined to the exceptional branch; inspect emitted code too.
#[cfg(feature = "bench-seed-div12")]
#[cold]
#[inline(never)]
fn mean_twelve_fallback(sum: u32) -> u32 {
    core::hint::black_box(sum) / 12
}
impl Seed {
    /// Remaining ARR relative to the measured edge, not acquisition return.
    /// Keep >=32us for the bounded MCU timer-arm sequence; refuse late seeds.
    pub fn handoff(&self, now: u32, wait: u32) -> Option<(u32, u16)> {
        self.handoff_with_min::<1333>(now, wait)
    }
    pub fn handoff_with_min<const MIN: u32>(&self, now: u32, wait: u32) -> Option<(u32, u16)> {
        self.handoff_with_budget::<MIN, 64>(now, wait)
    }
    /// Explicit adapter allowance, in half-us ticks. Keep the physical onset
    /// and reject invalid budgets; the caller must measure its complete arm.
    /// Existing callers retain64 ticks until a qualified adapter opts in.
    pub fn handoff_with_budget<const MIN: u32, const BUDGET: u32>(
        &self,
        now: u32,
        wait: u32,
    ) -> Option<(u32, u16)> {
        if BUDGET == 0 || BUDGET > u16::MAX as u32 {
            return None;
        }
        if (cfg!(feature = "bench-seed450") && MIN == 741)
            || (cfg!(feature = "bench-seed500") && MIN == 667)
        {
            if !(1..=6).contains(&self.step) || !(MIN..=2000).contains(&self.interval_ticks) {
                return None;
            }
            let age = now.wrapping_sub(self.edge_tick);
            let remaining = wait.checked_sub(age)?;
            return if (BUDGET..=u16::MAX as u32).contains(&remaining) {
                Some((age, remaining as u16))
            } else {
                None
            };
        }
        if !(matches!(MIN, 1010 | 1041 | 1075 | 1111 | 1333)
            || (cfg!(feature = "bench-seed400") && MIN == 834)
            || (cfg!(feature = "bench-range335") && MIN == 995)
            || (cfg!(feature = "bench-range340") && MIN == 980)
            || (cfg!(feature = "bench-range345") && MIN == 966)
            || (cfg!(feature = "bench-range350") && MIN == 952))
            || !(1..=6).contains(&self.step)
            || !(MIN..=2000).contains(&self.interval_ticks)
        {
            return None;
        }
        let age = now.wrapping_sub(self.edge_tick);
        let remaining = wait.checked_sub(age)?;
        if remaining < BUDGET || remaining > u16::MAX as u32 {
            return None;
        }
        Some((age, remaining as u16))
    }
}
/// Repeated sampled-level qualification, not proof of analog mux settling.
pub struct EdgeFilter {
    stable: Option<bool>,
    candidate: Option<(bool, u32)>,
    last: Option<u32>,
    pub cancelled: u32,
    pub max_gap: u32,
    failed: bool,
}
impl EdgeFilter {
    /// Confirm an existing candidate using consecutive reads. Preserve the
    /// sampled onset, enforce the existing gap bound, and never create an
    /// edge from the initial static level. Separate from sampled dwell mode.
    pub fn confirm_persistent(
        &mut self,
        proof: &PersistentLevel,
        tick: u32,
    ) -> Result<Option<(bool, u32)>, ()> {
        if self.failed {
            return Err(());
        }
        if let Some(last) = self.last {
            let gap = tick.wrapping_sub(last);
            self.max_gap = self.max_gap.max(gap);
            if gap > 200 {
                self.failed = true;
                return Err(());
            }
        }
        self.last = Some(tick);
        let Some((level, onset)) = self.candidate else {
            return Ok(None);
        };
        if level != proof.level {
            self.candidate = None;
            self.cancelled += 1;
            return Ok(None);
        }
        self.stable = Some(level);
        self.candidate = None;
        Ok(Some((level, onset)))
    }
    /// Fault-only snapshot: packed stable/candidate (2=absent), candidate onset,
    /// last sample. Acquisition is bounded below one16-bit half-us wrap.
    pub fn diagnostic(&self) -> [u16; 3] {
        let stable = self.stable.map_or(2, |v| v as u16);
        let candidate = self.candidate.map_or(2, |(v, _)| v as u16);
        [
            stable | (candidate << 2),
            self.candidate.map_or(0, |(_, t)| t as u16),
            self.last.unwrap_or(0) as u16,
        ]
    }
    pub fn new() -> Self {
        Self {
            stable: None,
            candidate: None,
            last: None,
            cancelled: 0,
            max_gap: 0,
            failed: false,
        }
    }
    pub fn sample(&mut self, level: bool, tick: u32) -> Result<Option<(bool, u32)>, ()> {
        self.sample_checked::<true>(level, tick)
    }
    pub fn sample_candidate(&mut self, level: bool, tick: u32) -> Result<Option<(bool, u32)>, ()> {
        self.sample_checked::<false>(level, tick)
    }
    fn sample_checked<const DWELL: bool>(
        &mut self,
        level: bool,
        tick: u32,
    ) -> Result<Option<(bool, u32)>, ()> {
        if self.failed {
            return Err(());
        }
        if let Some(last) = self.last {
            let gap = tick.wrapping_sub(last);
            self.max_gap = self.max_gap.max(gap);
            if gap > 200 {
                self.failed = true;
                return Err(());
            }
        }
        self.last = Some(tick);
        let Some(stable) = self.stable else {
            self.stable = Some(level);
            return Ok(None);
        };
        if level == stable {
            if self.candidate.take().is_some() {
                self.cancelled += 1;
            }
            return Ok(None);
        }
        if let Some((_, first)) = self.candidate {
            if DWELL && tick.wrapping_sub(first) >= 40 {
                self.stable = Some(level);
                self.candidate = None;
                return Ok(Some((level, first)));
            }
        } else {
            self.candidate = Some((level, tick));
        }
        Ok(None)
    }
}
pub type Acquire = Acquisition<8000, 666>;
#[cfg(all(test, feature = "bench-seed-timing-reanchor"))]
mod restart_tests {
    use super::*;
    #[test]
    fn poisoned_history_matches_fresh_object_and_original_deadline() {
        for origin in [0u32, u32::MAX - 2000] {
            for offset in [1000u32, 30000, 40001] {
                for first in 1..=6 {
                    for gap in [100u32, 1600, 2001] {
                        let mut used = RuntimeAcquire::new(origin);
                        used.previous = Some((6, 123));
                        used.intervals = 12;
                        used.sum = 24000;
                        used.fault = Some(Fault::TooSlow);
                        used.ready = Some(Seed {
                            step: 6,
                            edge_tick: 123,
                            interval_ticks: 2000,
                        });
                        used.wanted = 6;
                        used.skipped = 99;
                        used.qualification_waits = 88;
                        used.sectors = [u32::MAX, 0, 40000, 123, 999, 1];
                        used.seen = 63;
                        used.cycles = [7, 123, 999, 77];
                        used.restart_window();
                        assert_eq!(used.start, origin);
                        assert_eq!(used.sectors, [u32::MAX, 0, 40000, 123, 999, 1]);
                        let mut fresh = RuntimeAcquire::new(origin);
                        for i in 0..15u32 {
                            let step = (first - 1 + i) % 6 + 1;
                            let phase = [2, 0, 1, 2, 0, 1][(step - 1) as usize];
                            let tick = origin.wrapping_add(offset + i * gap);
                            assert_eq!(
                                used.edge(phase, step & 1 != 0, tick),
                                fresh.edge(phase, step & 1 != 0, tick)
                            );
                            assert_eq!(used.intervals(), fresh.intervals());
                            assert_eq!(used.cycles, fresh.cycles);
                            assert_eq!(used.last_edge(), fresh.last_edge());
                            assert_eq!(used.poll(tick), fresh.poll(tick));
                        }
                    }
                }
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acquisition<const MIN_CYCLE: u32, const MIN_INDIVIDUAL: u32> {
    start: u32,
    previous: Option<(u8, u32)>,
    intervals: u8,
    sum: u32,
    fault: Option<Fault>,
    ready: Option<Seed>,
    wanted: u8,
    pub skipped: u32,
    pub qualification_waits: u32,
    sectors: [u32; 6],
    seen: u8,
    /// Checked full cycles, minimum, maximum, rejected cycle (half-us ticks).
    pub cycles: [u32; 4],
}
/// A consumed, genuinely qualified acquisition followed by ONE further edge.
/// Pure protocol; the opt-in live adapter owns hardware. No timer/gate API here.
#[cfg(feature = "bench-reentry-next-edge")]
pub struct NextEdge<const MIN_CYCLE: u32, const MIN_INDIVIDUAL: u32> {
    acquire: Acquisition<MIN_CYCLE, MIN_INDIVIDUAL>,
    prior: Seed,
    terminal: Option<FollowFault>,
}
#[cfg(feature = "bench-reentry-next-edge")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowFault {
    Unqualified,
    Acquisition(Fault),
    Confirmation,
    Consumed,
}
#[cfg(feature = "bench-reentry-next-edge")]
#[derive(Debug, PartialEq, Eq)]
pub struct FollowedSeed {
    pub seed: Seed,
    pub prior_edge_tick: u32,
    pub follow_interval_ticks: u32,
}
/// Speculative policy work while the physical candidate is still confirming.
/// Exclusive borrowing prevents observation of speculative acquisition state.
/// Dropping this token rolls back; only finish after a real matching filtered
/// edge retains the update. This type has no timer/output authority.
#[cfg(feature = "bench-reentry-next-edge")]
pub struct PendingFollow<'a, const C: u32, const I: u32> {
    owner: &'a mut NextEdge<C, I>,
    original: Option<Acquisition<C, I>>,
    onset: u32,
    phase: u8,
    level: bool,
    result: Result<Seed, FollowFault>,
}
#[cfg(feature = "bench-reentry-next-edge")]
impl<const C: u32, const I: u32> PendingFollow<'_, C, I> {
    /// Caller must supply confirmation of the SAME phase/level/onset passed to
    /// prepare_edge, from the continuous EdgeFilter. A cancelled/replaced
    /// candidate must drop this token, never finish it with a different edge.
    pub fn finish(
        self,
        phase: u8,
        level: bool,
        onset: u32,
        confirmed: u32,
    ) -> Result<FollowedSeed, FollowFault> {
        self.finish_checked::<40>(phase, level, onset, confirmed)
    }
    pub fn finish_persistent(
        self,
        phase: u8,
        level: bool,
        onset: u32,
        confirmed: u32,
        proof: PersistentLevel,
    ) -> Result<FollowedSeed, FollowFault> {
        if proof.level != level {
            return Err(FollowFault::Confirmation);
        }
        self.finish_checked::<0>(phase, level, onset, confirmed)
    }
    fn finish_checked<const DWELL: u32>(
        mut self,
        phase: u8,
        level: bool,
        onset: u32,
        confirmed: u32,
    ) -> Result<FollowedSeed, FollowFault> {
        let refusal = if let Some(f) = self.owner.terminal {
            Some(f)
        } else if self.owner.prior.step == 0 {
            Some(FollowFault::Unqualified)
        } else if confirmed.wrapping_sub(self.owner.acquire.start) > 40_000 {
            Some(FollowFault::Acquisition(Fault::Expired))
        } else if (phase, level, onset) != (self.phase, self.level, self.onset)
            || !(DWELL..=240).contains(&confirmed.wrapping_sub(self.onset))
        {
            Some(FollowFault::Confirmation)
        } else {
            None
        };
        if let Some(f) = refusal {
            self.owner.terminal = Some(f);
            return Err(f); // Drop restores uncommitted acquisition state.
        }
        // All deadline/confirmation checks precede retention, just as edge().
        self.original = None;
        self.owner.terminal = Some(match self.result {
            Ok(_) => FollowFault::Consumed,
            Err(f) => f,
        });
        match self.result {
            Ok(Seed {
                step,
                edge_tick,
                interval_ticks,
            }) => Ok(FollowedSeed {
                seed: Seed {
                    step,
                    edge_tick,
                    interval_ticks,
                },
                prior_edge_tick: self.owner.prior.edge_tick,
                follow_interval_ticks: self.onset.wrapping_sub(self.owner.prior.edge_tick),
            }),
            Err(f) => Err(f),
        }
    }
}
#[cfg(feature = "bench-reentry-next-edge")]
impl<const C: u32, const I: u32> Drop for PendingFollow<'_, C, I> {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            self.owner.acquire = original;
        }
    }
}
#[cfg(feature = "bench-reentry-next-edge")]
impl<const C: u32, const I: u32> NextEdge<C, I> {
    pub fn prepare_candidate(
        &mut self,
        phase: usize,
        filter: &EdgeFilter,
    ) -> Option<PendingFollow<'_, C, I>> {
        if !self.final_candidate(phase, filter) {
            return None;
        }
        let (level, onset) = filter.candidate?;
        Some(self.prepare_edge(phase as u8, level, onset))
    }
    /// Perform onset-dependent policy work before the confirming sample.
    /// No qualified result escapes until finish; dropping restores every field.
    pub fn prepare_edge(&mut self, phase: u8, level: bool, onset: u32) -> PendingFollow<'_, C, I> {
        let (original, result) = if let Some(f) = self.terminal {
            (None, Err(f))
        } else if self.prior.step == 0 {
            (None, Err(FollowFault::Unqualified))
        } else {
            let original = self.acquire.clone();
            let result = match self.acquire.edge(phase, level, onset) {
                Ok(Some(seed)) => Ok(seed),
                Ok(None) => Err(FollowFault::Unqualified),
                Err(f) => Err(FollowFault::Acquisition(f)),
            };
            (Some(original), result)
        };
        PendingFollow {
            owner: self,
            original,
            onset,
            phase,
            level,
            result,
        }
    }
    /// Allocate this owner before sensing, then promote in place. Step0 is
    /// deliberately not a usable seed; only qualify() can leave this state.
    pub fn unqualified(acquire: Acquisition<C, I>) -> Self {
        Self {
            acquire,
            prior: Seed {
                step: 0,
                edge_tick: 0,
                interval_ticks: 0,
            },
            terminal: None,
        }
    }
    pub fn acquiring_mut(&mut self) -> Option<&mut Acquisition<C, I>> {
        if self.prior.step == 0 && self.terminal.is_none() {
            Some(&mut self.acquire)
        } else {
            None
        }
    }
    pub fn qualify(&mut self, now: u32) -> Result<Seed, FollowFault> {
        if let Some(f) = self.terminal {
            return Err(f);
        }
        let result = if self.prior.step != 0 {
            Err(FollowFault::Consumed)
        } else {
            self.acquire
                .poll(now)
                .map_err(FollowFault::Acquisition)
                .and_then(|seed| seed.ok_or(FollowFault::Unqualified))
        };
        match result {
            Ok(seed) => {
                self.prior = seed;
                self.acquire.ready = None;
                Ok(seed)
            }
            Err(f) => {
                self.terminal = Some(f);
                Err(f)
            }
        }
    }
    pub fn prior(&self) -> Seed {
        self.prior
    }
    pub fn final_candidate(&self, phase: usize, filter: &EdgeFilter) -> bool {
        if self.prior.step == 0 || self.terminal.is_some() || filter.failed {
            return false;
        }
        let step = if self.prior.step == 6 {
            1
        } else {
            self.prior.step + 1
        };
        let expected = [2usize, 0, 1, 2, 0, 1][(step - 1) as usize];
        phase == expected
            && filter.candidate.is_some_and(|(level, t)| {
                level == (step & 1 != 0)
                    && (I..=2000).contains(&t.wrapping_sub(self.prior.edge_tick))
            })
    }
    /// Keep using the physical acquisition clock, including time spent in setup.
    pub fn poll(&mut self, now: u32) -> Result<(), FollowFault> {
        if let Some(f) = self.terminal {
            return Err(f);
        }
        if self.prior.step == 0 {
            self.terminal = Some(FollowFault::Unqualified);
            return Err(FollowFault::Unqualified);
        }
        if let Err(f) = self.acquire.poll(now) {
            let f = FollowFault::Acquisition(f);
            self.terminal = Some(f);
            return Err(f);
        }
        Ok(())
    }
    /// Preserve the existing narrow grace for a real in-range candidate;
    /// strict poll() alone must not be substituted while it is confirming.
    pub fn poll_filtered(
        &mut self,
        now: u32,
        filters: &[EdgeFilter; 3],
    ) -> Result<(), FollowFault> {
        if let Some(f) = self.terminal {
            return Err(f);
        }
        if self.prior.step == 0 {
            self.terminal = Some(FollowFault::Unqualified);
            return Err(FollowFault::Unqualified);
        }
        if let Err(f) = self.acquire.poll_filtered(now, filters) {
            let f = FollowFault::Acquisition(f);
            self.terminal = Some(f);
            return Err(f);
        }
        Ok(())
    }
    /// Input must come from the same continuously serviced EdgeFilter stream.
    /// Preserve its onset timestamp; completion time only checks freshness and
    /// the ORIGINAL deadline. Never use the callback/return time as edge time.
    pub fn edge(
        &mut self,
        phase: u8,
        level: bool,
        onset: u32,
        confirmed: u32,
    ) -> Result<FollowedSeed, FollowFault> {
        self.edge_checked::<40>(phase, level, onset, confirmed)
    }
    pub fn edge_persistent(
        &mut self,
        phase: u8,
        level: bool,
        onset: u32,
        confirmed: u32,
        proof: PersistentLevel,
    ) -> Result<FollowedSeed, FollowFault> {
        if proof.level != level {
            self.terminal = Some(FollowFault::Confirmation);
            return Err(FollowFault::Confirmation);
        }
        self.edge_checked::<0>(phase, level, onset, confirmed)
    }
    fn edge_checked<const DWELL: u32>(
        &mut self,
        phase: u8,
        level: bool,
        onset: u32,
        confirmed: u32,
    ) -> Result<FollowedSeed, FollowFault> {
        if let Some(f) = self.terminal {
            return Err(f);
        }
        if self.prior.step == 0 {
            self.terminal = Some(FollowFault::Unqualified);
            return Err(FollowFault::Unqualified);
        }
        let result = (|| {
            if confirmed.wrapping_sub(self.acquire.start) > 40_000 {
                return Err(FollowFault::Acquisition(Fault::Expired));
            }
            // Same dwell40 plus maximum200tick next-visit allowance as existing
            // acquisition. This is not a new high-speed persistence policy.
            if !(DWELL..=240).contains(&confirmed.wrapping_sub(onset)) {
                return Err(FollowFault::Confirmation);
            }
            // Keep the scalar fields explicit across nested Result/Option.
            // On thumbv6m the combinator chain copied eleven unaligned payload
            // bytes twice through __aeabi_memcpy after edge confirmation.
            // Acquisition still performs every original check/history update.
            let (step, edge_tick, interval_ticks) = match self.acquire.edge(phase, level, onset) {
                Ok(Some(Seed {
                    step,
                    edge_tick,
                    interval_ticks,
                })) => (step, edge_tick, interval_ticks),
                Ok(None) => return Err(FollowFault::Unqualified),
                Err(f) => return Err(FollowFault::Acquisition(f)),
            };
            Ok(FollowedSeed {
                seed: Seed {
                    step,
                    edge_tick,
                    interval_ticks,
                },
                prior_edge_tick: self.prior.edge_tick,
                follow_interval_ticks: onset.wrapping_sub(self.prior.edge_tick),
            })
        })();
        self.terminal = Some(match &result {
            Ok(_) => FollowFault::Consumed,
            Err(f) => *f,
        });
        result
    }
}
impl<const MIN_CYCLE: u32, const MIN_INDIVIDUAL: u32> Acquisition<MIN_CYCLE, MIN_INDIVIDUAL> {
    /// Consume the completed acquisition, retaining its full cycle history and
    /// original deadline. The next edge keeps the measured twelve-interval mean
    /// as its provisional estimate; it is NOT advertised as a new13-interval mean.
    #[cfg(feature = "bench-reentry-next-edge")]
    pub fn into_next_edge(
        self,
        now: u32,
    ) -> Result<NextEdge<MIN_CYCLE, MIN_INDIVIDUAL>, FollowFault> {
        let mut next = NextEdge::unqualified(self);
        next.qualify(now)?;
        Ok(next)
    }
    pub fn new(start: u32) -> Self {
        assert!(
            matches!(
                (MIN_CYCLE, MIN_INDIVIDUAL),
                (8000, 666) | (6666, 554) | (6452, 536) | (6250, 520) | (6062, 504)
            ) || (cfg!(feature = "bench-range335") && (MIN_CYCLE, MIN_INDIVIDUAL) == (5972, 496))
                || (cfg!(feature = "bench-range340") && (MIN_CYCLE, MIN_INDIVIDUAL) == (5884, 490))
                || (cfg!(feature = "bench-range345") && (MIN_CYCLE, MIN_INDIVIDUAL) == (5798, 482))
                || (cfg!(feature = "bench-range350") && (MIN_CYCLE, MIN_INDIVIDUAL) == (5716, 476))
                || (cfg!(feature = "bench-seed400") && (MIN_CYCLE, MIN_INDIVIDUAL) == (5000, 476))
                || (cfg!(feature = "bench-seed450") && (MIN_CYCLE, MIN_INDIVIDUAL) == (4445, 476))
                || (cfg!(feature = "bench-seed500") && (MIN_CYCLE, MIN_INDIVIDUAL) == (4000, 476))
        );
        Self {
            start,
            previous: None,
            intervals: 0,
            sum: 0,
            fault: None,
            ready: None,
            wanted: 0,
            skipped: 0,
            qualification_waits: 0,
            sectors: [0; 6],
            seen: 0,
            cycles: [0; 4],
        }
    }
    /// Diagnostic selection of the FIRST real edge, followed by the unchanged
    /// twelve measured intervals. Never replace or refresh a completed seed.
    pub fn with_sector(start: u32, sector: u8) -> Option<Self> {
        if sector > 6 {
            return None;
        }
        let mut a = Self::new(start);
        a.wanted = sector;
        Some(a)
    }
    pub fn fault(&self) -> Option<Fault> {
        self.fault
    }
    /// Only the one-restart acquisition owner may discard a failed window.
    /// Retain original deadline; seen=0 makes every old sector timestamp inert.
    #[cfg(feature = "bench-seed-timing-reanchor")]
    pub(super) fn restart_window(&mut self) {
        self.previous = None;
        self.intervals = 0;
        self.sum = 0;
        self.fault = None;
        self.ready = None;
        self.wanted = 0;
        self.skipped = 0;
        self.qualification_waits = 0;
        self.seen = 0;
        self.cycles = [0; 4];
    }
    pub fn intervals(&self) -> u8 {
        self.intervals
    }
    pub fn last_edge(&self) -> Option<(u8, u32)> {
        self.previous
    }
    /// Scheduling hint ONLY, before the twelfth interval exists. Allows a
    /// caller to arrange output-disabled setup for the expected final sector.
    /// This is neither a Seed nor permission to drive or refresh timestamps.
    /// Caller must keep EdgeFilters sampled during preparation; edge() still
    /// validates the real final onset and original acquisition deadline.
    pub fn preparation_step(&self) -> Option<u8> {
        if self.intervals != 11 || self.ready.is_some() || self.fault.is_some() {
            return None;
        }
        self.previous
            .map(|(step, _)| if step == 6 { 1 } else { step + 1 })
    }
    /// Scheduling hint only: caller may revisit the final candidate sooner,
    /// but must still use EdgeFilter::sample and edge for qualification.
    pub fn final_candidate(&self, phase: usize, filter: &EdgeFilter) -> bool {
        if self.intervals != 11 || self.ready.is_some() || self.fault.is_some() || filter.failed {
            return false;
        }
        let Some((step, last)) = self.previous else {
            return false;
        };
        let (expected, level) = match step % 6 + 1 {
            1 => (2, true),
            2 => (0, false),
            3 => (1, true),
            4 => (2, false),
            5 => (0, true),
            _ => (1, false),
        };
        phase == expected
            && filter.candidate.is_some_and(|(v, t)| {
                v == level && (MIN_INDIVIDUAL..=2000).contains(&t.wrapping_sub(last))
            })
    }
    fn fail(&mut self, reason: Fault) -> Result<Option<Seed>, Fault> {
        self.fault = Some(reason);
        self.ready = None;
        Err(reason)
    }
    pub fn poll(&mut self, now: u32) -> Result<Option<Seed>, Fault> {
        if let Some(f) = self.fault {
            return Err(f);
        }
        if now.wrapping_sub(self.start) > 40_000 {
            return self.fail(Fault::Expired);
        }
        if let Some((_, last)) = self.previous {
            if now.wrapping_sub(last) > 2000 {
                return self.fail(Fault::TooSlow);
            }
        }
        Ok(self.ready)
    }
    /// Wall-time polling must allow the expected edge's bounded qualification
    /// latency. Physical onset intervals remain checked by edge()/poll().
    /// Only an existing in-range candidate can defer TooSlow; no blind grace.
    pub fn poll_filtered(
        &mut self,
        now: u32,
        filters: &[EdgeFilter; 3],
    ) -> Result<Option<Seed>, Fault> {
        if self.fault.is_none() && self.ready.is_none() && now.wrapping_sub(self.start) <= 40_000 {
            if let Some((step, last)) = self.previous {
                if now.wrapping_sub(last) <= 2000 {
                    return self.poll(now);
                }
                let (phase, level) = match step % 6 + 1 {
                    1 => (2, true),
                    2 => (0, false),
                    3 => (1, true),
                    4 => (2, false),
                    5 => (0, true),
                    _ => (1, false),
                };
                let f = &filters[phase];
                if let (Some((value, first)), Some(sample)) = (f.candidate, f.last) {
                    // Dwell40 + at most200 ticks to the next confirming visit.
                    // A missing visit, cancelled pulse or late onset cannot pass.
                    if !f.failed
                        && value == level
                        && (MIN_INDIVIDUAL..=2000).contains(&first.wrapping_sub(last))
                        && now.wrapping_sub(first) <= 240
                        && now.wrapping_sub(sample) <= 200
                    {
                        self.qualification_waits += 1;
                        return Ok(None);
                    }
                }
            }
        }
        self.poll(now)
    }
    pub fn edge(&mut self, phase: u8, level: bool, tick: u32) -> Result<Option<Seed>, Fault> {
        self.poll(tick)?;
        // One-shot result: later traffic cannot refresh the seed timestamp.
        // Caller must consume immediately or poll to verify its age.
        if self.ready.is_some() {
            return Ok(self.ready);
        }
        // Reference sequence on this verified physical rotation:
        // C up, A down, B up, C down, A up, B down.
        let step = match (phase, level) {
            (2, true) => 1,
            (0, false) => 2,
            (1, true) => 3,
            (2, false) => 4,
            (0, true) => 5,
            (1, false) => 6,
            _ => return self.fail(Fault::InvalidPhase),
        };
        if self.previous.is_none() && self.wanted != 0 && step != self.wanted {
            self.skipped += 1;
            return Ok(None);
        }
        if let Some((prior, last)) = self.previous {
            if step != prior % 6 + 1 {
                return self.fail(Fault::WrongOrder);
            }
            let gap = tick.wrapping_sub(last);
            if gap < MIN_INDIVIDUAL {
                return self.fail(Fault::TooFast);
            }
            // Individual sectors may be unequal. Every repeated sector must
            // independently satisfy the 4..6ms electrical-cycle envelope.
            let index = (step - 1) as usize;
            if self.seen & (1 << index) != 0 {
                let cycle = tick.wrapping_sub(self.sectors[index]);
                self.cycles[0] += 1;
                self.cycles[1] = if self.cycles[0] == 1 {
                    cycle
                } else {
                    self.cycles[1].min(cycle)
                };
                self.cycles[2] = self.cycles[2].max(cycle);
                if !(MIN_CYCLE..=12000).contains(&cycle) {
                    self.cycles[3] = cycle;
                    return self.fail(if cycle < MIN_CYCLE {
                        Fault::CycleTooFast
                    } else {
                        Fault::CycleTooSlow
                    });
                }
            }
            if self.intervals < 12 {
                self.sum += gap;
                self.intervals += 1;
            }
            if self.intervals == 12 && self.ready.is_none() {
                #[cfg(feature = "bench-seed-div12")]
                let mean = mean_twelve(self.sum);
                #[cfg(not(feature = "bench-seed-div12"))]
                let mean = self.sum / 12;
                self.ready = Some(Seed {
                    step,
                    edge_tick: tick,
                    interval_ticks: mean,
                });
            }
        }
        self.sectors[(step - 1) as usize] = tick;
        self.seen |= 1 << (step - 1);
        self.previous = Some((step, tick));
        Ok(self.ready)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "bench-seed500")]
    #[test]
    fn seed500_twenty_us_reserve_keeps_absolute_deadline() {
        const ARM_LIMIT_US: u32 = 16;
        const SLACK_US: u32 = 4;
        const BUDGET: u32 = (ARM_LIMIT_US + SLACK_US) * 2;
        const _: () = assert!(BUDGET == 40);
        for origin in [100u32, u32::MAX - 80] {
            let seed = super::Seed {
                step: 1,
                edge_tick: origin,
                interval_ticks: 667,
            };
            for age in 0..=200u32 {
                let result = seed.handoff_with_budget::<667, BUDGET>(origin.wrapping_add(age), 167);
                if age <= 127 {
                    assert_eq!(result, Some((age, (167 - age) as u16)));
                    let (_, remaining) = result.unwrap();
                    assert_eq!(age + remaining as u32, 167);
                    assert!(remaining as u32 - ARM_LIMIT_US * 2 >= SLACK_US * 2);
                } else {
                    assert_eq!(result, None);
                }
            }
            assert_eq!(
                super::Seed {
                    interval_ticks: 666,
                    ..seed
                }
                .handoff_with_budget::<667, BUDGET>(origin, 167),
                None
            );
        }
    }
    #[cfg(feature = "bench-seed500")]
    #[test]
    fn seed500_requires_twelve_intervals_and_exact_arm_budget() {
        assert_eq!(
            (
                super::SEED_MIN_TICKS,
                super::CYCLE_MIN_TICKS,
                super::INDIVIDUAL_MIN_TICKS
            ),
            (667, 4000, 476)
        );
        let seed = super::Seed {
            step: 1,
            edge_tick: 100,
            interval_ticks: 667,
        };
        assert_eq!(
            seed.handoff_with_min::<667>(100 + 103, 167),
            Some((103, 64))
        );
        assert_eq!(seed.handoff_with_min::<667>(100 + 104, 167), None);
        assert_eq!(
            super::Seed {
                interval_ticks: 666,
                ..seed
            }
            .handoff_with_min::<667>(100, 167),
            None
        );
        let mut acquire = super::RuntimeAcquire::new(0);
        for i in 0..=12u32 {
            let step = (i % 6 + 1) as u8;
            let phase = [2, 0, 1, 2, 0, 1][(step - 1) as usize];
            assert_eq!(
                acquire
                    .edge(phase, step & 1 != 0, i * 667)
                    .unwrap()
                    .is_some(),
                i == 12
            );
        }
    }
    #[cfg(all(feature = "bench-seed450", not(feature = "bench-seed500")))]
    #[test]
    fn seed450_domain_keeps_physical_deadline_and_arm_reserve() {
        assert_eq!(
            (
                super::SEED_MIN_TICKS,
                super::CYCLE_MIN_TICKS,
                super::INDIVIDUAL_MIN_TICKS
            ),
            (741, 4445, 476)
        );
        let seed = super::Seed {
            step: 1,
            edge_tick: 100,
            interval_ticks: 741,
        };
        assert_eq!(
            seed.handoff_with_min::<741>(100 + 121, 185),
            Some((121, 64))
        );
        assert_eq!(seed.handoff_with_min::<741>(100 + 122, 185), None);
        assert_eq!(
            super::Seed {
                interval_ticks: 740,
                ..seed
            }
            .handoff_with_min::<741>(100, 185),
            None
        );
        let mut acquire = super::RuntimeAcquire::new(0);
        let mut ready = None;
        for i in 0..=12u32 {
            let step = (i % 6 + 1) as u8;
            let phase = [2, 0, 1, 2, 0, 1][(step - 1) as usize];
            ready = acquire.edge(phase, step & 1 != 0, i * 741).unwrap();
            assert_eq!(ready.is_some(), i == 12);
        }
        assert_eq!(ready.unwrap().interval_ticks, 741);
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn direct_persistence_matches_transaction_for_valid_confirmations() {
        for age in [0u32, 6, 40, 240] {
            let source = qualified_follow_source(0, 0, 1);
            let prior = source.ready.unwrap();
            let onset = prior.edge_tick + 1400;
            let mut direct = source.clone().into_next_edge(prior.edge_tick + 40).unwrap();
            let mut transactional = source.into_next_edge(prior.edge_tick + 40).unwrap();
            let proof = super::persistent_level(false, || false).unwrap();
            let expected = transactional
                .prepare_edge(0, false, onset)
                .finish_persistent(0, false, onset, onset + age, proof);
            let proof = super::persistent_level(false, || false).unwrap();
            let actual = direct.edge_persistent(0, false, onset, onset + age, proof);
            assert_eq!(actual, expected);
            assert!(actual.is_ok());
            assert_eq!(direct.acquire, transactional.acquire);
            assert_eq!(direct.terminal, transactional.terminal);
        }
    }
    #[test]
    fn consecutive_persistence_rejects_every_flip_without_extra_reads() {
        for reject in 0..12 {
            let mut reads = 0;
            let got = super::persistent_level(true, || {
                let value = reads != reject;
                reads += 1;
                value
            });
            assert!(got.is_none());
            assert_eq!(reads, reject + 1);
        }
        for level in [false, true] {
            let mut reads = 0;
            assert!(
                super::persistent_level(level, || {
                    reads += 1;
                    level
                })
                .is_some()
            );
            assert_eq!(reads, 12);
        }
    }
    #[test]
    fn persistent_confirmation_keeps_candidate_onset_and_gap_checks() {
        for origin in [100u32, u32::MAX - 20] {
            let mut filter = super::EdgeFilter::new();
            let proof = super::persistent_level(false, || false).unwrap();
            assert_eq!(filter.confirm_persistent(&proof, origin), Ok(None));
            assert_eq!(filter.sample(true, origin.wrapping_add(2)), Ok(None));
            assert_eq!(filter.sample(false, origin.wrapping_add(4)), Ok(None));
            let proof = super::persistent_level(false, || false).unwrap();
            assert_eq!(
                filter.confirm_persistent(&proof, origin.wrapping_add(10)),
                Ok(Some((false, origin.wrapping_add(4))))
            );
            let proof = super::persistent_level(false, || false).unwrap();
            assert_eq!(
                filter.confirm_persistent(&proof, origin.wrapping_add(211)),
                Err(())
            );
        }
    }
    #[test]
    fn explicit_arm_budget_preserves_onset_and_exact_deadline() {
        use super::Seed;
        for onset in [100u32, u32::MAX - 50] {
            let seed = Seed {
                step: 1,
                edge_tick: onset,
                interval_ticks: 1400,
            };
            for age in 0..401u32 {
                let now = onset.wrapping_add(age);
                let remaining = 350u32.checked_sub(age);
                assert_eq!(
                    seed.handoff_with_min::<1333>(now, 350),
                    seed.handoff_with_budget::<1333, 64>(now, 350)
                );
                let got = seed.handoff_with_budget::<1333, 40>(now, 350);
                assert_eq!(got.is_some(), remaining.is_some_and(|r| r >= 40));
                if let Some((actual_age, arr)) = got {
                    assert_eq!(actual_age, age);
                    assert_eq!(now.wrapping_add(arr as u32), onset.wrapping_add(350));
                }
            }
            assert_eq!(seed.handoff_with_budget::<1333, 0>(onset, 350), None);
            assert_eq!(seed.handoff_with_budget::<1333, 65536>(onset, 350), None);
            assert_eq!(seed.handoff_with_budget::<1332, 40>(onset, 350), None);
        }
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn prepared_follow_matches_original_results_and_entire_state() {
        fn check<const C: u32, const I: u32>(spacing: u32) {
            for origin in [0u32, u32::MAX - 5000] {
                for first in 1..=6 {
                    for phase in 0..=3 {
                        for level in [false, true] {
                            for gap in [0u32, 665, 666, 1200, 1400, 2000, 2001, 30000] {
                                for age in [0u32, 39, 40, 240, 241] {
                                    let mut source = super::Acquisition::<C, I>::new(origin);
                                    for i in 0..=12u32 {
                                        let step = ((first - 1 + i) % 6 + 1) as u8;
                                        let p = [2, 0, 1, 2, 0, 1][(step - 1) as usize];
                                        source
                                            .edge(
                                                p,
                                                step & 1 != 0,
                                                origin.wrapping_add(i * spacing),
                                            )
                                            .unwrap();
                                    }
                                    let prior = source.ready.unwrap();
                                    let mut old =
                                        source.clone().into_next_edge(prior.edge_tick).unwrap();
                                    let mut staged =
                                        source.into_next_edge(prior.edge_tick).unwrap();
                                    let onset = prior.edge_tick.wrapping_add(gap);
                                    let confirmed = onset.wrapping_add(age);
                                    let expected = old.edge(phase, level, onset, confirmed);
                                    let actual = staged
                                        .prepare_edge(phase, level, onset)
                                        .finish(phase, level, onset, confirmed);
                                    assert_eq!(actual, expected);
                                    assert_eq!(staged.acquire, old.acquire);
                                    assert_eq!(staged.terminal, old.terminal);
                                    assert_eq!(staged.prior, old.prior);
                                    assert_eq!(
                                        staged.edge(phase, level, onset, confirmed),
                                        old.edge(phase, level, onset, confirmed)
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        check::<8000, 666>(1400);
        #[cfg(feature = "bench-range350")]
        check::<5716, 476>(1000);
        #[cfg(feature = "bench-seed400")]
        check::<5000, 476>(900);
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn cancelled_prevalidation_restores_all_history_and_can_retry() {
        use super::*;
        for gap in [666, 1400, 2001] {
            let source = qualified_follow_source(0, 0, 1);
            let prior = source.ready.unwrap();
            let mut next = source.into_next_edge(prior.edge_tick).unwrap();
            let saved = next.acquire.clone();
            drop(next.prepare_edge(0, false, prior.edge_tick + gap));
            assert_eq!(next.acquire, saved);
            assert_eq!(next.terminal, None);
            let onset = prior.edge_tick + 1400;
            assert!(
                next.prepare_edge(0, false, onset)
                    .finish(0, false, onset, onset + 40)
                    .is_ok()
            );
        }
        let mut unqualified = NextEdge::unqualified(Acquire::new(0));
        drop(unqualified.prepare_edge(0, false, 100));
        assert!(unqualified.acquiring_mut().is_some());
        assert_eq!(
            unqualified
                .prepare_edge(0, false, 100)
                .finish(0, false, 100, 140),
            Err(FollowFault::Unqualified)
        );
        for (phase, level, delta) in [(1, false, 0), (0, true, 0), (0, false, 1)] {
            let source = qualified_follow_source(0, 0, 1);
            let prior = source.ready.unwrap();
            let onset = prior.edge_tick + 1400;
            let mut next = source.into_next_edge(prior.edge_tick).unwrap();
            let saved = next.acquire.clone();
            assert_eq!(
                next.prepare_edge(0, false, onset)
                    .finish(phase, level, onset + delta, onset + 40),
                Err(FollowFault::Confirmation)
            );
            assert_eq!(next.acquire, saved);
        }
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn inplace_qualification_preserves_storage_history_and_deadline() {
        use super::*;
        for origin in [0u32, u32::MAX - 5000] {
            for first in 1..=6u8 {
                let mut next = NextEdge::unqualified(Acquire::new(origin));
                let address = next.acquiring_mut().unwrap() as *mut Acquire;
                let mut expected = None;
                for i in 0..=12u32 {
                    let step = ((first as u32 - 1 + i) % 6 + 1) as u8;
                    let phase = [2, 0, 1, 2, 0, 1][(step - 1) as usize];
                    let a = next.acquiring_mut().unwrap();
                    assert_eq!(address, a as *mut Acquire);
                    expected = a
                        .edge(phase, step & 1 != 0, origin.wrapping_add(i * 1400))
                        .unwrap();
                }
                let seed = expected.unwrap();
                let before = (
                    next.acquire.start,
                    next.acquire.sectors,
                    next.acquire.cycles,
                    next.acquire.sum,
                );
                assert_eq!(next.qualify(seed.edge_tick.wrapping_add(40)), Ok(seed));
                assert!(next.acquiring_mut().is_none());
                assert!(next.acquire.ready.is_none());
                assert_eq!(address, &mut next.acquire as *mut Acquire);
                assert_eq!(
                    before,
                    (
                        next.acquire.start,
                        next.acquire.sectors,
                        next.acquire.cycles,
                        next.acquire.sum
                    )
                );
                let step = if first == 6 { 1 } else { first + 1 };
                let phase = [2, 0, 1, 2, 0, 1][(step - 1) as usize];
                let onset = seed.edge_tick.wrapping_add(1400);
                let result = next
                    .edge(phase, step & 1 != 0, onset, onset.wrapping_add(40))
                    .unwrap();
                assert_eq!(
                    result.seed,
                    Seed {
                        step,
                        edge_tick: onset,
                        interval_ticks: 1400
                    }
                );
                assert!(next.qualify(onset).is_err());
                assert!(next.acquiring_mut().is_none());
            }
        }
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn inplace_unqualified_calls_latch_and_cannot_be_promoted_again() {
        use super::*;
        for operation in 0..4 {
            let mut next = NextEdge::unqualified(Acquire::new(0));
            let filters = core::array::from_fn(|_| EdgeFilter::new());
            assert!(!next.final_candidate(0, &filters[0]));
            let failed = match operation {
                0 => next.poll(0).unwrap_err(),
                1 => next.poll_filtered(0, &filters).unwrap_err(),
                2 => next.edge(2, true, 0, 40).err().unwrap(),
                _ => next.qualify(0).unwrap_err(),
            };
            assert_eq!(failed, FollowFault::Unqualified);
            assert!(next.acquiring_mut().is_none());
            assert_eq!(next.qualify(40001), Err(FollowFault::Unqualified));
        }
        let mut next = qualified_follow_source(0, 0, 1)
            .into_next_edge(16840)
            .unwrap();
        assert_eq!(next.qualify(16841), Err(FollowFault::Consumed));
        assert_eq!(next.poll(16842), Err(FollowFault::Consumed));
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    fn qualified_follow_source(start: u32, first: u32, first_step: u8) -> super::Acquire {
        let mut a = super::Acquire::new(start);
        for i in 0..=12u32 {
            let step = ((u32::from(first_step) - 1 + i) % 6 + 1) as u8;
            let phase = [2, 0, 1, 2, 0, 1][(step - 1) as usize];
            let result = a
                .edge(phase, step & 1 != 0, first.wrapping_add(i * 1400))
                .unwrap();
            assert_eq!(result.is_some(), i == 12);
        }
        a
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn next_edge_keeps_order_history_original_mean_and_real_onset() {
        use super::*;
        for start in [0, u32::MAX - 5000] {
            for step in 1..=6 {
                let a = qualified_follow_source(start, start, step);
                let prior = a.ready.unwrap();
                let old_cycles = a.cycles[0];
                let mut next = a.into_next_edge(prior.edge_tick.wrapping_add(80)).unwrap();
                let following = if step == 6 { 1 } else { step + 1 };
                let phase = [2, 0, 1, 2, 0, 1][(following - 1) as usize];
                let onset = prior.edge_tick.wrapping_add(1500);
                let result = next
                    .edge(phase, following & 1 != 0, onset, onset.wrapping_add(40))
                    .unwrap();
                assert_eq!(
                    result.seed,
                    Seed {
                        step: following,
                        edge_tick: onset,
                        interval_ticks: 1400
                    }
                );
                assert_eq!(
                    (result.prior_edge_tick, result.follow_interval_ticks),
                    (prior.edge_tick, 1500)
                );
                assert_eq!(next.acquire.cycles[0], old_cycles + 1);
                assert_eq!(
                    next.edge(phase, following & 1 != 0, onset + 1, onset + 41),
                    Err(FollowFault::Consumed)
                );
                assert_eq!(next.poll(onset), Err(FollowFault::Consumed));
            }
        }
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn next_edge_refusals_latch_and_do_not_reset_deadline() {
        use super::*;
        assert!(matches!(
            Acquire::new(0).into_next_edge(0),
            Err(FollowFault::Unqualified)
        ));
        for (phase, level, gap, age, expected) in [
            (0, false, 1400, 39, FollowFault::Confirmation),
            (0, false, 1400, 241, FollowFault::Confirmation),
            (
                1,
                true,
                1400,
                40,
                FollowFault::Acquisition(Fault::WrongOrder),
            ),
            (0, false, 665, 40, FollowFault::Acquisition(Fault::TooFast)),
            (
                0,
                false,
                666,
                40,
                FollowFault::Acquisition(Fault::CycleTooFast),
            ),
            (0, false, 2001, 40, FollowFault::Acquisition(Fault::TooSlow)),
        ] {
            let a = qualified_follow_source(0, 0, 1);
            let last = a.ready.unwrap().edge_tick;
            let mut next = a.into_next_edge(last + 80).unwrap();
            assert_eq!(
                next.edge(phase, level, last + gap, last + gap + age),
                Err(expected)
            );
            assert_eq!(next.poll(last + 100), Err(expected));
        }
        let a = qualified_follow_source(0, 23200, 1);
        let mut next = a.into_next_edge(40000).unwrap();
        assert_eq!(
            next.edge(0, false, 41400, 41440),
            Err(FollowFault::Acquisition(Fault::Expired))
        );
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn next_edge_keeps_only_real_candidate_grace() {
        use super::*;
        let a = qualified_follow_source(0, 0, 1);
        let last = a.ready.unwrap().edge_tick;
        let mut next = a.into_next_edge(last + 80).unwrap();
        let mut filters = core::array::from_fn(|_| EdgeFilter::new());
        filters[0].sample(true, last + 1900).unwrap();
        filters[0].sample(false, last + 2000).unwrap();
        assert_eq!(next.poll_filtered(last + 2010, &filters), Ok(()));
        let (level, onset) = filters[0].sample(false, last + 2040).unwrap().unwrap();
        assert!(next.edge(0, level, onset, last + 2040).is_ok());
        let a = qualified_follow_source(0, 0, 1);
        let mut next = a.into_next_edge(last + 80).unwrap();
        let filters = core::array::from_fn(|_| EdgeFilter::new());
        assert_eq!(
            next.poll_filtered(last + 2010, &filters),
            Err(FollowFault::Acquisition(Fault::TooSlow))
        );
    }
    #[test]
    fn range330_preserves_seed_age_and_cycle_limits() {
        use super::*;
        let seed = Seed {
            step: 1,
            edge_tick: 100,
            interval_ticks: 1010,
        };
        let wait = minz_core::am32::wait_time(1010, minz_core::am32::advance_of(1010, 16));
        assert_eq!(wait, 253);
        assert_eq!(seed.handoff_with_min::<1010>(289, wait), Some((189, 64)));
        assert_eq!(seed.handoff_with_min::<1010>(290, wait), None);
        // Observed87us would leave39.5us, but the live check remains authoritative.
        assert_eq!(seed.handoff_with_min::<1010>(274, wait), Some((174, 79)));
        for spacing in [1010, 1011] {
            let mut a = Acquisition::<6062, 504>::with_sector(0, 0).unwrap();
            for n in 0..6 {
                let (p, l) = EDGES[n];
                assert!(a.edge(p, l, 100 + n as u32 * spacing).is_ok());
            }
            let (p, l) = EDGES[0];
            let result = a.edge(p, l, 100 + 6 * spacing);
            if spacing == 1010 {
                assert_eq!(result, Err(Fault::CycleTooFast));
            } else {
                assert!(result.is_ok());
            }
        }
    }
    #[cfg(feature = "bench-seed-div12")]
    #[test]
    fn bounded_seed_division_is_exact_with_full_width_fallback() {
        for sum in 0..=24_000 {
            assert_eq!(super::mean_twelve(sum), sum / 12);
        }
        for sum in [
            24_001,
            24_011,
            24_012,
            32_767,
            65_535,
            u32::MAX - 1,
            u32::MAX,
        ] {
            assert_eq!(super::mean_twelve(sum), sum / 12);
        }
    }
    #[test]
    fn measured_seed_age_limits_prospective_speed_profiles() {
        // E368-369: 91us observed edge-to-arm, NOT a worst-case bound.
        // This evaluates reference arithmetic only; no new profile is enabled.
        let measured_age = 182;
        for (hz, ci, wait, remaining) in [
            (320, 1041, 260, 78),
            (325, 1025, 256, 74),
            (330, 1010, 253, 71),
            (340, 980, 245, 63),
        ] {
            assert_eq!(2_000_000 / (6 * hz), ci);
            let actual = minz_core::am32::wait_time(
                ci,
                minz_core::am32::advance_of(ci, minz_core::am32_loop::TEMP_ADVANCE),
            );
            assert_eq!(actual, wait);
            assert_eq!(actual - measured_age, remaining);
            assert_eq!(remaining >= 64, hz < 340);
        }
        // Existing admission still refuses prospective intervals below1041,
        // even when the hypothetical arm arithmetic has sufficient time.
        let seed = super::Seed {
            step: 1,
            edge_tick: 100,
            interval_ticks: 1010,
        };
        assert_eq!(seed.handoff_with_min::<1041>(282, 253), None);
        assert_eq!(seed.handoff_with_min::<1009>(282, 253), None);
    }
    #[test]
    fn lean_seed_age_still_bounds_expansion_without_granting_authority() {
        // E378-382 observed87us, not WCET. These are prospective arithmetic
        // cases only: neither profile support nor runtime limits are expanded.
        const AGE_TICKS: u32 = 87 * 2;
        const ARM_FLOOR: u32 = 32 * 2;
        for (hz, ci, remaining) in [
            (330, 1010, 79),
            (340, 980, 71),
            (345, 966, 68),
            (350, 952, 64),
            (351, 949, 63),
        ] {
            assert_eq!(2_000_000 / (6 * hz), ci);
            let wait = minz_core::am32::wait_time(
                ci,
                minz_core::am32::advance_of(ci, minz_core::am32_loop::TEMP_ADVANCE),
            );
            assert_eq!(wait - AGE_TICKS, remaining);
            assert_eq!(remaining >= ARM_FLOOR, hz <= 350);
            if hz > 330 {
                let seed = super::Seed {
                    step: 1,
                    edge_tick: 100,
                    interval_ticks: ci,
                };
                assert_eq!(seed.handoff_with_min::<1010>(100 + AGE_TICKS, wait), None);
            }
        }
        // Merely five additional microseconds invalidate the prospective340
        // boundary. Nominal slack cannot be promoted to a worst-case guarantee.
        let wait = minz_core::am32::wait_time(980, minz_core::am32::advance_of(980, 16));
        assert!(wait - (AGE_TICKS + 10) < ARM_FLOOR);
    }
    #[test]
    fn measured85us_age_bounds_prospective355_without_authority() {
        // E436-439: observed edge-to-arm age, not WCET or new admission.
        for (hz, ci, wait) in [
            (350, 952, 238),
            (355, 938, 235),
            (356, 936, 234),
            (357, 933, 233),
            (360, 925, 231),
        ] {
            assert_eq!(2_000_000 / (6 * hz), ci);
            let actual = minz_core::am32::wait_time(
                ci,
                minz_core::am32::advance_of(ci, minz_core::am32_loop::TEMP_ADVANCE),
            );
            assert_eq!(actual, wait);
            assert_eq!(actual - 170 >= 64, hz <= 356);
            if hz > 350 {
                // Existing350 admission must still refuse these intervals.
                let seed = Seed {
                    step: 1,
                    edge_tick: 100,
                    interval_ticks: ci,
                };
                assert_eq!(seed.handoff_with_min::<952>(270, actual), None);
            }
        }
        //355 has just0.5us nominal margin: one extra us refuses.
        assert_eq!(235 - 170, 65);
        assert!(235 - 172 < 64);
    }
    #[test]
    fn prospective335_budget_is_not_current_profile_authority() {
        // Offline boundary study only. No new accepted MIN or cycle profile.
        let ci = 2_000_000 / (6 * 335);
        assert_eq!(ci, 995);
        let wait = minz_core::am32::wait_time(
            ci,
            minz_core::am32::advance_of(ci, minz_core::am32_loop::TEMP_ADVANCE),
        );
        assert_eq!(wait, 249);
        assert_eq!(wait - 85 * 2, 79); //39.5us,7.5us nominal margin
        assert_eq!(wait - 87 * 2, 75); //37.5us,5.5us nominal margin
        assert_eq!(wait - 92 * 2, 65);
        assert!(wait - 93 * 2 < 64); //6us beyond87us consumes all margin
        let seed = Seed {
            step: 1,
            edge_tick: 100,
            interval_ticks: ci,
        };
        assert_eq!(seed.handoff_with_min::<1010>(270, wait), None);
        if !cfg!(feature = "bench-range335") {
            assert_eq!(seed.handoff_with_min::<995>(270, wait), None);
        }
    }
    #[cfg(feature = "bench-range350")]
    #[test]
    fn range350_refuses_extra_half_us_at_arm_boundary() {
        if !cfg!(feature = "bench-seed400") {
            assert_eq!(
                (SEED_MIN_TICKS, CYCLE_MIN_TICKS, INDIVIDUAL_MIN_TICKS),
                (952, 5716, 476)
            );
        }
        let seed = Seed {
            step: 1,
            edge_tick: u32::MAX - 99,
            interval_ticks: 952,
        };
        let wait = minz_core::am32::wait_time(952, minz_core::am32::advance_of(952, 16));
        assert_eq!(wait, 238);
        assert_eq!(seed.handoff_with_min::<952>(74, wait), Some((174, 64)));
        assert_eq!(seed.handoff_with_min::<952>(75, wait), None);
        assert_eq!(seed.handoff_with_min::<966>(70, wait), None);
        for spacing in [952, 953] {
            let mut a = Acquisition::<5716, 476>::with_sector(0, 0).unwrap();
            for n in 0..6 {
                let (p, l) = EDGES[n];
                assert_eq!(a.edge(p, l, 100 + n as u32 * spacing), Ok(None));
            }
            let (p, l) = EDGES[0];
            let r = a.edge(p, l, 100 + 6 * spacing);
            if spacing == 952 {
                assert_eq!(r, Err(Fault::CycleTooFast));
            } else {
                assert_eq!(r, Ok(None));
                for n in 7..12 {
                    let (p, l) = EDGES[n % 6];
                    assert_eq!(a.edge(p, l, 100 + n as u32 * spacing), Ok(None));
                }
                assert_eq!(
                    a.edge(p, l, 100 + 12 * spacing)
                        .unwrap()
                        .unwrap()
                        .interval_ticks,
                    953
                );
            }
        }
    }
    #[cfg(feature = "bench-range345")]
    #[test]
    fn range345_preserves_live_arm_and_cycle_floor() {
        if !cfg!(feature = "bench-range350") {
            assert_eq!(
                (SEED_MIN_TICKS, CYCLE_MIN_TICKS, INDIVIDUAL_MIN_TICKS),
                (966, 5798, 482)
            );
        }
        let seed = Seed {
            step: 1,
            edge_tick: u32::MAX - 99,
            interval_ticks: 966,
        };
        let wait = minz_core::am32::wait_time(966, minz_core::am32::advance_of(966, 16));
        assert_eq!(wait, 242);
        assert_eq!(seed.handoff_with_min::<966>(78, wait), Some((178, 64)));
        assert_eq!(seed.handoff_with_min::<966>(79, wait), None);
        assert_eq!(seed.handoff_with_min::<980>(70, wait), None);
        for spacing in [966, 967] {
            let mut a = Acquisition::<5798, 482>::with_sector(0, 0).unwrap();
            for n in 0..6 {
                let (p, l) = EDGES[n];
                assert_eq!(a.edge(p, l, 100 + n as u32 * spacing), Ok(None));
            }
            let (p, l) = EDGES[0];
            let r = a.edge(p, l, 100 + 6 * spacing);
            if spacing == 966 {
                assert_eq!(r, Err(Fault::CycleTooFast));
            } else {
                assert_eq!(r, Ok(None));
                for n in 7..12 {
                    let (p, l) = EDGES[n % 6];
                    assert_eq!(a.edge(p, l, 100 + n as u32 * spacing), Ok(None));
                }
                assert_eq!(
                    a.edge(p, l, 100 + 12 * spacing)
                        .unwrap()
                        .unwrap()
                        .interval_ticks,
                    967
                );
            }
        }
    }
    #[cfg(feature = "bench-range340")]
    #[test]
    fn range340_preserves_live_arm_floor_and_full_sequence() {
        if !cfg!(feature = "bench-range345") {
            assert_eq!(
                (SEED_MIN_TICKS, CYCLE_MIN_TICKS, INDIVIDUAL_MIN_TICKS),
                (980, 5884, 490)
            );
        }
        let seed = Seed {
            step: 1,
            edge_tick: u32::MAX - 99,
            interval_ticks: 980,
        };
        let wait = minz_core::am32::wait_time(980, minz_core::am32::advance_of(980, 16));
        assert_eq!(wait, 245);
        assert_eq!(seed.handoff_with_min::<980>(81, wait), Some((181, 64)));
        assert_eq!(seed.handoff_with_min::<980>(82, wait), None);
        assert_eq!(seed.handoff_with_min::<995>(70, wait), None);
        for spacing in [980, 981] {
            let mut a = Acquisition::<5884, 490>::with_sector(0, 0).unwrap();
            for n in 0..6 {
                let (p, l) = EDGES[n];
                assert_eq!(a.edge(p, l, 100 + n as u32 * spacing), Ok(None));
            }
            let (p, l) = EDGES[0];
            let r = a.edge(p, l, 100 + 6 * spacing);
            if spacing == 980 {
                assert_eq!(r, Err(Fault::CycleTooFast));
            } else {
                assert_eq!(r, Ok(None));
                for n in 7..12 {
                    let (p, l) = EDGES[n % 6];
                    assert_eq!(a.edge(p, l, 100 + n as u32 * spacing), Ok(None));
                }
                assert_eq!(
                    a.edge(p, l, 100 + 12 * spacing)
                        .unwrap()
                        .unwrap()
                        .interval_ticks,
                    981
                );
            }
        }
    }
    #[cfg(feature = "bench-range335")]
    #[test]
    fn range335_expands_profile_not_arm_or_acquisition_safeguards() {
        if !cfg!(feature = "bench-range340") {
            assert_eq!(
                (SEED_MIN_TICKS, CYCLE_MIN_TICKS, INDIVIDUAL_MIN_TICKS),
                (995, 5972, 496)
            );
        }
        let seed = Seed {
            step: 1,
            edge_tick: u32::MAX - 99,
            interval_ticks: 995,
        };
        let wait = minz_core::am32::wait_time(995, minz_core::am32::advance_of(995, 16));
        assert_eq!(wait, 249);
        assert_eq!(seed.handoff_with_min::<995>(85, wait), Some((185, 64)));
        assert_eq!(seed.handoff_with_min::<995>(86, wait), None);
        assert_eq!(seed.handoff_with_min::<1010>(70, wait), None);
        for spacing in [995, 996] {
            let mut a = Acquisition::<5972, 496>::with_sector(0, 0).unwrap();
            for n in 0..6 {
                let (p, l) = EDGES[n];
                assert_eq!(a.edge(p, l, 100 + n as u32 * spacing), Ok(None));
            }
            let (p, l) = EDGES[0];
            let r = a.edge(p, l, 100 + 6 * spacing);
            if spacing == 995 {
                assert_eq!(r, Err(Fault::CycleTooFast));
            } else {
                assert_eq!(r, Ok(None));
                for n in 7..12 {
                    let (p, l) = EDGES[n % 6];
                    assert_eq!(a.edge(p, l, 100 + n as u32 * spacing), Ok(None));
                }
                assert_eq!(
                    a.edge(p, l, 100 + 12 * spacing)
                        .unwrap()
                        .unwrap()
                        .interval_ticks,
                    996
                );
            }
        }
    }
    #[test]
    fn range320_keeps_actual_arm_and_cycle_refusals() {
        use super::*;
        let seed = Seed {
            step: 1,
            edge_tick: 100,
            interval_ticks: 1041,
        };
        let wait = minz_core::am32::wait_time(1041, minz_core::am32::advance_of(1041, 16));
        assert_eq!(wait, 260);
        assert_eq!(seed.handoff_with_min::<1041>(296, wait), Some((196, 64)));
        assert_eq!(seed.handoff_with_min::<1041>(297, wait), None);
        for spacing in [1041, 1042] {
            let mut a = Acquisition::<6250, 520>::with_sector(0, 0).unwrap();
            for n in 0..6 {
                let (p, l) = EDGES[n];
                assert!(a.edge(p, l, 100 + n as u32 * spacing).is_ok());
            }
            let (p, l) = EDGES[0];
            let r = a.edge(p, l, 100 + 6 * spacing);
            if spacing == 1041 {
                assert_eq!(r, Err(Fault::CycleTooFast));
            } else {
                assert!(r.is_ok());
            }
        }
    }
    use super::*;
    const EDGES: [(u8, bool); 6] = [
        (2, true),
        (0, false),
        (1, true),
        (2, false),
        (0, true),
        (1, false),
    ];
    #[test]
    fn range310_keeps_twelve_edges_and_actual_arm_floor() {
        let mut a = Acquisition::<6452, 536>::with_sector(0, 0).unwrap();
        for n in 0..=12 {
            let (phase, level) = EDGES[n % 6];
            let result = a.edge(phase, level, 100 + n as u32 * 1076).unwrap();
            if n < 12 {
                assert!(result.is_none());
            } else {
                let seed = result.unwrap();
                let wait = seed.interval_ticks / 2 - ((seed.interval_ticks * 16) >> 6);
                assert_eq!(
                    seed.handoff_with_min::<1075>(seed.edge_tick + 205, wait),
                    Some((205, 64))
                );
                assert_eq!(
                    seed.handoff_with_min::<1075>(seed.edge_tick + 206, wait),
                    None
                );
                assert_eq!(
                    seed.handoff_with_min::<1075>(seed.edge_tick + 212, wait),
                    None
                );
            }
        }
        let mut a = Acquisition::<6452, 536>::with_sector(0, 0).unwrap();
        for n in 0..6 {
            let (p, l) = EDGES[n];
            assert!(a.edge(p, l, 100 + n as u32 * 1075).is_ok());
        }
        let (p, l) = EDGES[0];
        assert_eq!(a.edge(p, l, 100 + 6 * 1075), Err(Fault::CycleTooFast));
    }
    #[test]
    fn experimental_acquisition_requires_full_sequence_and_fresh_handoff() {
        // E201 measured7578tick cycle; equal sectors below are SYNTHETIC,
        // not reconstruction of the missing edge-by-edge physical record.
        type Faster = Acquisition<6666, 554>;
        for start in [0, u32::MAX - 1000] {
            let mut a = Faster::new(start);
            for i in 0..13 {
                let (p, l) = EDGES[i % 6];
                let result = a.edge(p, l, start.wrapping_add(i as u32 * 1263)).unwrap();
                assert_eq!(result.is_some(), i == 12);
                if let Some(seed) = result {
                    assert_eq!(a.cycles, [7, 7578, 7578, 0]);
                    assert_eq!(seed.handoff(seed.edge_tick, 315), None);
                    assert_eq!(
                        seed.handoff_with_min::<1111>(seed.edge_tick.wrapping_add(251), 315),
                        Some((251, 64))
                    );
                    assert_eq!(
                        seed.handoff_with_min::<1111>(seed.edge_tick.wrapping_add(252), 315),
                        None
                    );
                }
            }
        }
        let mut a = Faster::new(0);
        for i in 0..6 {
            let (p, l) = EDGES[i];
            a.edge(p, l, i as u32 * 1110).unwrap();
        }
        assert_eq!(a.edge(2, true, 6660), Err(Fault::CycleTooFast));
        let mut a = Faster::new(0);
        a.edge(2, true, 0).unwrap();
        assert_eq!(a.edge(0, false, 553), Err(Fault::TooFast));
        let mut a = Faster::new(0);
        a.edge(2, true, 0).unwrap();
        assert_eq!(a.edge(1, true, 1263), Err(Fault::WrongOrder));
        let mut a = Faster::new(0);
        a.edge(2, true, 0).unwrap();
        assert_eq!(a.poll(2001), Err(Fault::TooSlow));
    }
    #[test]
    fn recorded_e190_missing_candidate_must_not_get_blind_grace() {
        // recovery46_all62_01 F85: last step4/edge6950,decision8972;
        // A low/no candidate/sample8940,B high/sample8972,C low/sample8876.
        // The earlier edge history is synthetic: only the saved final state
        // is known. Do not present this as replay of an unrecorded waveform.
        let mut a = Acquire::new(0);
        for (p, l, t) in [
            (1, false, 500),
            (2, true, 1850),
            (0, false, 3550),
            (1, true, 5200),
            (2, false, 6950),
        ] {
            assert_eq!(a.edge(p, l, t), Ok(None));
        }
        let mut filters = core::array::from_fn(|_| EdgeFilter::new());
        assert_eq!(filters[0].sample(false, 8940), Ok(None));
        assert_eq!(filters[1].sample(true, 8972), Ok(None));
        assert_eq!(filters[2].sample(false, 8876), Ok(None));
        assert_eq!(filters[0].diagnostic(), [8, 0, 8940]);
        assert_eq!(a.last_edge(), Some((4, 6950)));
        assert_eq!(a.intervals(), 4);
        assert_eq!(a.poll_filtered(8972, &filters), Err(Fault::TooSlow));
        assert_eq!(a.qualification_waits, 0);
        // A hypothetical in-range edge cannot be inserted after refusal to
        // manufacture a seed. It was not observed in this retained snapshot.
        assert_eq!(a.edge(0, true, 8948), Err(Fault::TooSlow));
        assert_eq!(a.intervals(), 4);
    }
    #[test]
    fn final_candidate_hint_cannot_bypass_dwell_cancellation_or_order() {
        let mut a = Acquire::new(0);
        for i in 0..12 {
            let (p, l) = EDGES[i % 6];
            a.edge(p, l, i as u32 * 1400).unwrap();
        }
        let mut f = EdgeFilter::new();
        f.sample(false, 16750).unwrap();
        f.sample(true, 16800).unwrap();
        assert!(a.final_candidate(2, &f));
        assert!(!a.final_candidate(0, &f));
        assert_eq!(f.sample(true, 16839), Ok(None));
        assert_eq!(f.sample(false, 16840), Ok(None));
        assert!(!a.final_candidate(2, &f));
        assert_eq!(a.intervals(), 11);
        f.sample(true, 16860).unwrap();
        let (level, t) = f.sample(true, 16900).unwrap().unwrap();
        let seed = a.edge(2, level, t).unwrap().unwrap();
        assert_eq!(seed.edge_tick, 16860);
        assert_eq!(a.cycles[0], 7);
        assert!(!a.final_candidate(2, &f));
    }
    #[test]
    fn wall_poll_can_expire_while_in_range_edge_awaits_qualification() {
        let mut a = Acquire::new(0);
        a.edge(2, true, 0).unwrap();
        let mut f = EdgeFilter::new();
        // A falling edge starts at1950ticks (<2000), but its second sample
        // comes100ticks later (within the200tick sampling-gap contract).
        for t in (0..=1900).step_by(100) {
            f.sample(true, t).unwrap();
        }
        assert_eq!(f.sample(false, 1950), Ok(None));
        assert_eq!(f.diagnostic(), [1, 1950, 1950]);
        // Another phase visit polls wall time before A's second sample.
        assert_eq!(a.poll(2001), Err(Fault::TooSlow));
        assert_eq!(f.sample(false, 2050), Ok(Some((false, 1950))));
        assert_eq!(a.edge(0, false, 1950), Err(Fault::TooSlow));
    }
    #[test]
    fn filtered_poll_preserves_in_range_candidate_until_confirmation() {
        // E134/acq_snapshot_06: step6 at20468, C candidate22428,
        // another-phase decision22488. Onset980us, wall age1010us.
        let mut a = Acquire::new(0);
        a.edge(1, false, 20468).unwrap();
        let mut fs = core::array::from_fn(|_| EdgeFilter::new());
        fs[2].sample(false, 22328).unwrap();
        fs[2].sample(true, 22428).unwrap();
        assert_eq!(a.poll_filtered(22488, &fs), Ok(None));
        assert_eq!(a.qualification_waits, 1);
        let (v, t) = fs[2].sample(true, 22528).unwrap().unwrap();
        assert_eq!(a.edge(2, v, t), Ok(None));
        assert_eq!(a.intervals(), 1);
        assert_eq!(a.last_edge(), Some((1, 22428)));
    }
    #[test]
    fn filtered_poll_cannot_extend_missing_late_cancelled_or_stale_edges() {
        for kind in 0..5 {
            let mut a = Acquire::new(0);
            a.edge(2, true, 0).unwrap();
            let mut fs = core::array::from_fn(|_| EdgeFilter::new());
            fs[0].sample(true, 1900).unwrap();
            match kind {
                0 => {} // no candidate
                1 => {
                    fs[0].sample(false, 2001).unwrap();
                } // late physical onset
                2 => {
                    fs[0].sample(false, 1950).unwrap();
                    fs[0].sample(true, 2000).unwrap();
                }
                3 => {
                    fs[1].sample(false, 1900).unwrap();
                    fs[1].sample(true, 1950).unwrap();
                } // wrong phase
                _ => {
                    fs[0].sample(false, 1950).unwrap();
                } // absent confirming sample
            }
            let now = if kind == 4 { 2151 } else { 2020 };
            assert_eq!(a.poll_filtered(now, &fs), Err(Fault::TooSlow));
        }
    }
    #[test]
    fn filtered_poll_keeps_absolute_deadline_and_wrap_contract() {
        for start in [0, u32::MAX - 1000] {
            let mut a = Acquire::new(start);
            a.edge(2, true, start).unwrap();
            let mut fs = core::array::from_fn(|_| EdgeFilter::new());
            fs[0].sample(true, start.wrapping_add(1900)).unwrap();
            fs[0].sample(false, start.wrapping_add(1950)).unwrap();
            assert_eq!(a.poll_filtered(start.wrapping_add(2020), &fs), Ok(None));
            assert_eq!(
                a.poll_filtered(start.wrapping_add(40001), &fs),
                Err(Fault::Expired)
            );
        }
    }
    #[test]
    fn selected_seed_requires_twelve_new_intervals_without_resetting_deadline() {
        for wanted in 1..=6u8 {
            let mut a = Acquire::with_sector(0, wanted).unwrap();
            let count = wanted as usize + 12;
            for i in 0..count {
                let (p, l) = EDGES[i % 6];
                let seed = a.edge(p, l, 100 + i as u32 * 1667).unwrap();
                if i + 1 == count {
                    let seed = seed.unwrap();
                    assert_eq!(seed.step, wanted);
                    assert_eq!(seed.edge_tick, 100 + i as u32 * 1667);
                    assert_eq!(seed.interval_ticks, 1667);
                    assert_eq!(a.intervals(), 12);
                } else {
                    assert!(seed.is_none());
                }
            }
            assert_eq!(a.skipped, wanted as u32 - 1);
        }
        let mut a = Acquire::with_sector(0, 3).unwrap();
        assert_eq!(a.edge(2, true, 100).unwrap(), None);
        assert_eq!(a.poll(40001), Err(Fault::Expired));
        assert!(Acquire::with_sector(0, 7).is_none());
    }
    #[test]
    fn two_cycles_seed_from_observed_time_and_sector() {
        for offset in 0..6 {
            let mut a = Acquire::new(0);
            for i in 0..13 {
                let (p, l) = EDGES[(i + offset) % 6];
                let seed = a.edge(p, l, 100 + i as u32 * 1667).unwrap();
                assert_eq!(seed.is_some(), i == 12);
            }
            assert_eq!(
                a.poll(20104).unwrap(),
                Some(Seed {
                    step: offset as u8 + 1,
                    edge_tick: 20104,
                    interval_ticks: 1667
                })
            );
        }
    }
    #[test]
    fn missing_reversed_duplicate_and_spike_edges_latch_faults() {
        for (p, l, t, fault) in [
            (2, true, 1667, Fault::WrongOrder),
            (1, true, 1667, Fault::WrongOrder),
            (0, false, 300, Fault::TooFast),
            (0, false, 2001, Fault::TooSlow),
            (9, false, 1667, Fault::InvalidPhase),
        ] {
            let mut a = Acquire::new(0);
            a.edge(2, true, 0).unwrap();
            assert_eq!(a.edge(p, l, t), Err(fault));
            assert_eq!(a.edge(0, false, 1667), Err(fault));
            assert!(a.ready.is_none());
        }
    }
    #[test]
    fn timeout_without_input_and_wrap_are_explicit() {
        let mut a = Acquire::new(7);
        assert_eq!(a.poll(40008), Err(Fault::Expired));
        let start = u32::MAX - 500;
        let mut a = Acquire::new(start);
        a.edge(2, true, start).unwrap();
        a.edge(0, false, start.wrapping_add(1667)).unwrap();
        assert_eq!(a.intervals(), 1);
    }
    #[test]
    fn average_cannot_hide_out_of_envelope_interval() {
        let mut a = Acquire::new(0);
        a.edge(2, true, 0).unwrap();
        a.edge(0, false, 2000).unwrap();
        assert_eq!(a.edge(1, true, 2665), Err(Fault::TooFast));
    }
    #[test]
    fn unequal_sectors_require_seven_valid_full_cycle_checks() {
        for start in [0, u32::MAX - 500] {
            let mut a = Acquire::new(start);
            let mut tick = start;
            let gaps = [1284, 1430, 1430, 1430, 1430, 1432]; //8436ticks, ~237eHz
            for i in 0..13 {
                let (p, l) = EDGES[i % 6];
                let seed = a.edge(p, l, tick).unwrap();
                assert_eq!(seed.is_some(), i == 12);
                if let Some(s) = seed {
                    assert_eq!(s.interval_ticks, 1406);
                }
                tick = tick.wrapping_add(gaps[i % 6]);
            }
            assert_eq!(a.cycles, [7, 8436, 8436, 0]);
        }
    }
    #[test]
    fn full_cycle_rejects_uniform_overspeed_before_seed() {
        let mut a = Acquire::new(0);
        for i in 0..6 {
            let (p, l) = EDGES[i];
            assert_eq!(a.edge(p, l, i as u32 * 1320), Ok(None));
        }
        assert_eq!(a.edge(2, true, 7920), Err(Fault::CycleTooFast));
        assert_eq!(a.cycles, [1, 7920, 7920, 7920]);
        assert_eq!(a.poll(8000), Err(Fault::CycleTooFast));
    }
    #[test]
    fn later_edges_cannot_keep_an_old_seed_fresh() {
        let mut a = Acquire::new(0);
        for i in 0..13 {
            let (p, l) = EDGES[i % 6];
            a.edge(p, l, i as u32 * 1667).unwrap();
        }
        a.edge(0, false, 21671).unwrap();
        assert_eq!(a.poll(22005), Err(Fault::TooSlow));
    }
    #[test]
    fn initial_level_and_short_pulse_do_not_become_edges() {
        let mut f = EdgeFilter::new();
        assert_eq!(f.sample(false, 0), Ok(None));
        assert_eq!(f.sample(true, 30), Ok(None));
        assert_eq!(f.sample(false, 50), Ok(None));
        assert_eq!(f.cancelled, 1);
        assert_eq!(f.sample(true, 80), Ok(None));
        assert_eq!(f.sample(true, 119), Ok(None));
        assert_eq!(f.sample(true, 120), Ok(Some((true, 80))));
        assert_eq!(f.sample(true, 130), Ok(None));
    }
    #[test]
    fn sampling_gap_fault_cannot_confirm_a_stale_candidate() {
        let mut f = EdgeFilter::new();
        f.sample(false, 0).unwrap();
        f.sample(true, 20).unwrap();
        assert_eq!(f.sample(true, 221), Err(()));
        assert_eq!(f.sample(false, 222), Err(()));
    }
    #[test]
    fn sequential_mux_samples_acquire_all_initial_sectors() {
        // 15us per mux visit, 45us per phase: baseline levels are not edges.
        // Edge confirmation is backdated to the first differing sample.
        for offset in 0..6 {
            let mut levels = [false; 3];
            for i in 0..6 {
                let (p, l) = EDGES[(offset + i) % 6];
                levels[p as usize] = l;
            }
            let mut filters: [EdgeFilter; 3] = core::array::from_fn(|_| EdgeFilter::new());
            let mut acquire = Acquire::new(0);
            let mut next_edge = 500;
            let mut edge_index = offset;
            let mut result = None;
            for visit in 0..1000 {
                let tick = visit * 30;
                while tick >= next_edge {
                    let (p, l) = EDGES[edge_index % 6];
                    levels[p as usize] = l;
                    edge_index += 1;
                    next_edge += 1667;
                }
                let phase = (visit % 3) as usize;
                result = if let Some((level, first)) =
                    filters[phase].sample(levels[phase], tick).unwrap()
                {
                    acquire.edge(phase as u8, level, first).unwrap()
                } else {
                    acquire.poll(tick).unwrap()
                };
                if result.is_some() {
                    break;
                }
            }
            let seed = result.expect("sampled two-cycle seed");
            assert_eq!(seed.step, offset as u8 + 1);
            assert!((1660..=1675).contains(&seed.interval_ticks));
            assert_eq!(acquire.intervals(), 12);
            assert!(filters.iter().all(|f| f.max_gap == 90));
        }
    }
    #[test]
    fn handoff_accounts_for_edge_age_and_refuses_late_seed() {
        let s = Seed {
            step: 2,
            edge_tick: 20424,
            interval_ticks: 1687,
        };
        assert_eq!(s.handoff(20528, 422), Some((104, 318)));
        assert_eq!(s.handoff(20782, 422), Some((358, 64)));
        assert_eq!(s.handoff(20783, 422), None);
        assert_eq!(s.handoff(20423, 422), None);
        assert_eq!(s.handoff(30000, 422), None);
    }
    #[test]
    fn reverse_startup_budget_is_bounded_by_measured_arm() {
        let s = Seed {
            step: 1,
            edge_tick: 100,
            interval_ticks: 1666,
        };
        assert_eq!(s.handoff_with_budget::<952, 40>(366, 308), Some((266, 42)));
        assert_eq!(s.handoff_with_budget::<952, 40>(368, 308), Some((268, 40)));
        assert_eq!(s.handoff_with_budget::<952, 40>(369, 308), None);
        assert_eq!(s.handoff_with_budget::<952, 64>(366, 308), None);
    }
}
