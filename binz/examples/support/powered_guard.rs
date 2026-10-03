//! Fail-closed envelope for bounded powered-reference experiments.
//! Pure policy only: caller must apply a fault immediately to DRV safing.
//! Absolute phase pulse limits are NOT average-current/duty-expansion approval.
use super::accepted_timing;
pub const CAMPAIGN_BASE_US: u32 = if cfg!(feature = "bench-hold-30s") {
    30_000_000
} else {
    5_000_000
};
/// Reserve two guard ticks for dispatch/safing latency inside the user limit.
pub fn reserved_elapsed(elapsed: u32) -> Option<u32> {
    elapsed.checked_add(200).filter(|&n| n < CAMPAIGN_BASE_US)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    CampaignDeadline,
    SegmentDeadline,
    TickGap,
    FeedbackStale,
    Current,
    Bus,
    Driver,
    Tracking,
    HostAbort,
    InvalidSeed,
    AdcTimeout,
    CycleTiming,
}
/// Absolute elapsed-time accounting across a single optional re-entry.
/// This is a budget, NOT permission to wake or drive. The caller must retain
/// this object from first handoff; reconstructing it on retry defeats the cap.
pub struct SessionBudget {
    campaign_limit: u32,
    powered_end: u32,
    first_elapsed: u32,
    reentry_used: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub elapsed: u32,
    pub campaign: u32,
    pub segment: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReentryRefusal {
    WrongFault,
    AlreadyAttempted,
    Deadline,
}
impl SessionBudget {
    pub fn initial(startup_elapsed: u32, window: u32) -> Option<(Self, Limits)> {
        let elapsed = reserved_elapsed(startup_elapsed)?;
        if !(20_000..=600_000_000).contains(&window) {
            return None;
        }
        let campaign = CAMPAIGN_BASE_US + window;
        Some((
            Self {
                campaign_limit: campaign,
                powered_end: startup_elapsed + window,
                first_elapsed: startup_elapsed,
                reentry_used: false,
            },
            Limits {
                elapsed,
                campaign,
                segment: window,
            },
        ))
    }
    /// Count shutdown, wake, acquisition and setup against both ORIGINAL
    /// deadlines. Reserve200us before the original powered deadline too.
    /// A refused attempt cannot be retried indefinitely under this budget.
    pub fn reentry(&mut self, elapsed: u32, prior_fault: Fault) -> Result<Limits, ReentryRefusal> {
        self.reentry_reserved::<200>(elapsed, prior_fault)
    }
    /// Reserve is subtracted from available drive time,never added to deadline.
    /// The size-optimized driven recovery also budgets foreground return cost.
    pub fn reentry_reserved<const RESERVE: u32>(
        &mut self,
        elapsed: u32,
        prior_fault: Fault,
    ) -> Result<Limits, ReentryRefusal> {
        if self.reentry_used {
            return Err(ReentryRefusal::AlreadyAttempted);
        }
        self.reentry_used = true;
        if prior_fault != Fault::Tracking {
            return Err(ReentryRefusal::WrongFault);
        }
        if elapsed < self.first_elapsed {
            return Err(ReentryRefusal::Deadline);
        }
        if !matches!(RESERVE, 200 | 300) {
            return Err(ReentryRefusal::Deadline);
        }
        let elapsed = elapsed
            .checked_add(RESERVE)
            .ok_or(ReentryRefusal::Deadline)?;
        let segment = self
            .powered_end
            .checked_sub(elapsed)
            .filter(|&n| n >= 20_000)
            .ok_or(ReentryRefusal::Deadline)?;
        if elapsed >= self.campaign_limit {
            return Err(ReentryRefusal::Deadline);
        }
        Ok(Limits {
            elapsed,
            campaign: self.campaign_limit,
            segment,
        })
    }
}
#[derive(Clone, Copy)]
pub struct Feedback {
    pub phase: [u16; 3],
    pub bus_mv: u32,
    pub vref: u16,
}
/// Call only after admission has validated a sector in1..=6. Keep the
/// successor exact without a general remainder sequence in the installer.
const fn next_valid_step(step: u8) -> u8 {
    if step == 6 { 1 } else { step + 1 }
}
const _: () = {
    let mut step = 1;
    while step <= 6 {
        assert!(next_valid_step(step) == step % 6 + 1);
        step += 1;
    }
};
pub fn phase_valid(phase: [u16; 3]) -> bool {
    // With the signed average-current owner, the raw frame was already checked
    // for VREF validity and accumulated before this generic guard. Phase codes
    // are bounded 12-bit ADC data; 0/4095 are reported saturation, not a second
    // instantaneous-current veto. Do not shadow the owner with the historical
    // tuned +/-1200-count threshold or an exact-rail duplicate.
    #[cfg(any(feature = "bench-startup-adc", feature = "bench-average-current"))]
    {
        return phase.iter().all(|&v| v <= 4095);
    }
    #[cfg(not(any(feature = "bench-startup-adc", feature = "bench-average-current")))]
    {
        // Exactly abs(v-2048)<=1200; its range already excludes ADC rails.
        // Explicit three channels avoid an opt-s iterator/stack loop on M0.
        const fn within(v: u16) -> bool {
            v.wrapping_sub(848) <= 2400
        }
        within(phase[0]) && within(phase[1]) && within(phase[2])
    }
}
pub fn validate_feedback(f: Feedback) -> Result<(), Fault> {
    if !phase_valid(f.phase) {
        return Err(Fault::Current);
    }
    if f.vref == 0
        || f.vref >= 4095
        || (!cfg!(feature = "bench-average-current") && f.bus_mv < 8400)
    {
        return Err(Fault::Bus);
    }
    Ok(())
}
/// Division-free guard decision for a coherent raw DMA frame. This is used by
/// lean interrupt paths; foreground diagnostics may still convert to mV.
pub fn validate_raw_feedback(raw: [u16; 5], vcal: u32) -> Result<Feedback, Fault> {
    let phase = [raw[0], raw[1], raw[2]];
    if !phase_valid(phase) {
        return Err(Fault::Current);
    }
    let vref = raw[4];
    if vref == 0 || vref >= 4095 || vcal == 0 || vcal > 4095 {
        return Err(Fault::Bus);
    }
    // Existing conversion accepts at divider-node code >=704. Requiring
    // bus*vcal/vref >=963 implies >=704 even after its integer VDDA floor.
    // It never accepts a frame rejected by the old >=8400mV decision and is
    // only a few raw counts conservative. Products fit u32 at full ADC scale.
    if !cfg!(feature = "bench-average-current") && raw[3] as u32 * vcal < 963u32 * vref as u32 {
        return Err(Fault::Bus);
    }
    Ok(Feedback {
        phase,
        bus_mv: 8400,
        vref,
    })
}
#[cfg(test)]
mod phase_tests {
    #[test]
    fn irq_and_foreground_share_exact_phase_limits() {
        for raw in 0..=u16::MAX {
            for phase in 0..3 {
                let mut values = [2048; 3];
                values[phase] = raw;
                assert_eq!(super::phase_valid(values), (848..=3248).contains(&raw));
            }
        }
    }
    fn scale(n: u32) -> u32 {
        n * 1194 / 100
    }
    fn legacy_bus(raw: u16, vref: u16, vcal: u32) -> bool {
        if vref == 0 || vref >= 4095 || vcal == 0 || vcal > 4095 {
            return false;
        }
        let vdda = 3000 * vcal / vref as u32;
        scale(raw as u32 * vdda / 4096) >= 8400
    }
    #[test]
    fn raw_bus_guard_never_accepts_legacy_undervoltage() {
        for vcal in [1, 100, 1000, 1200, 1500, 1800, 2000, 3000, 4095] {
            for vref in 1..4095u16 {
                let boundary = (963 * vref as u32 + vcal - 1) / vcal;
                for bus in boundary.saturating_sub(3)..=boundary.saturating_add(3).min(4095) {
                    let raw = [2048, 2048, 2048, bus as u16, vref];
                    if super::validate_raw_feedback(raw, vcal).is_ok() {
                        assert!(
                            legacy_bus(bus as u16, vref, vcal),
                            "vcal={vcal} vref={vref} bus={bus}"
                        );
                    }
                }
            }
        }
    }
}
/// Currently bench-qualified running envelope. Startup acquisition is separate.
pub type Guard = RunGuard<4000, 333>;
/// Three controller half-microsecond periods expressed in microseconds.
/// Split at667 so the only multiplication has an operand <=666: the product
/// is <=1998 and compiles to native 32-bit MUL on ARMv6-M, never wide math.
pub const fn speed_event_limit_us(reference_half_us: u32) -> u32 {
    if reference_half_us >= 667 {
        1000
    } else {
        let us = (reference_half_us * 3 + 1) >> 1;
        if us < 200 { 200 } else { us }
    }
}
const _: () = {
    assert!(speed_event_limit_us(0) == 200);
    assert!(speed_event_limit_us(133) == 200);
    assert!(speed_event_limit_us(200) == 300);
    assert!(speed_event_limit_us(666) == 999);
    assert!(speed_event_limit_us(667) == 1000);
    assert!(speed_event_limit_us(u32::MAX) == 1000);
};
/// Compile-time experimental envelope: minimum full-cycle and inter-event us.
/// Instantiating another profile is NOT hardware qualification. Keep the1ms
/// missing-event bound and6ms slow-cycle bound independent of this speed cap.
/// Const parameters add no per-instance RAM or runtime profile-change path.
pub struct RunGuard<
    const MIN_CYCLE_US: u32,
    const MIN_EVENT_US: u32,
    const REPORT_FAST: bool = false,
> {
    start: u32,
    campaign_elapsed: u32,
    last_poll: u32,
    last_feedback: u32,
    first_step: Option<u8>,
    events: accepted_timing::Monitor,
    fault: Option<Fault>,
    cycle_at: [u32; 6],
    cycle_seen: u8,
    campaign_limit: u32,
    segment_limit: u32,
    fast_count: u32,
    fast_min: u32,
    #[cfg(feature = "bench-reentry-next-edge")]
    followed_seed: bool,
}
/// Single-use immediate installation inputs, NOT authority to enable hardware.
/// Private fields and profile parameters prevent forging or cross-profile use.
/// Caller must not retain this across time/ownership transitions: timestamps
/// are deliberately never refreshed by installation.
#[cfg(feature = "bench-guard-install")]
pub struct Admission<const MIN_CYCLE_US: u32, const MIN_EVENT_US: u32> {
    now: u32,
    elapsed: u32,
    campaign: u32,
    segment: u32,
    feedback_at: u32,
    step: u8,
}
impl<const MIN_CYCLE_US: u32, const MIN_EVENT_US: u32, const REPORT_FAST: bool>
    RunGuard<MIN_CYCLE_US, MIN_EVENT_US, REPORT_FAST>
{
    /// Cold construction while the hardware owner is inactive. The sentinel
    /// is deliberately unhealthy; it cannot authorize poll/commit/feedback.
    /// Caller must revoke its separate one-shot staging token on shutdown.
    #[cfg(feature = "bench-reentry-guard-stage")]
    pub fn stage_install(slot: &mut Option<Self>) {
        *slot = Some(Self {
            start: 0,
            campaign_elapsed: 0,
            last_poll: 0,
            last_feedback: 0,
            first_step: None,
            events: accepted_timing::Monitor::new(0, MIN_EVENT_US, 1000),
            fault: Some(Fault::InvalidSeed),
            cycle_at: [0; 6],
            cycle_seen: 0,
            campaign_limit: 0,
            segment_limit: 0,
            fast_count: 0,
            fast_min: u32::MAX,
            #[cfg(feature = "bench-reentry-next-edge")]
            followed_seed: false,
        });
    }
    /// Consume fresh admission without reclearing cold cycle/history storage.
    /// This does not refresh admission timestamps or validate hardware ownership.
    #[cfg(feature = "bench-reentry-guard-stage")]
    pub fn install_staged(
        slot: &mut Option<Self>,
        a: Admission<MIN_CYCLE_US, MIN_EVENT_US>,
    ) -> bool {
        let Some(g) = slot.as_mut() else {
            return false;
        };
        if g.fault != Some(Fault::InvalidSeed) || g.first_step.is_some() {
            return false;
        }
        g.start = a.now;
        g.campaign_elapsed = a.elapsed;
        g.last_poll = a.now;
        g.last_feedback = a.feedback_at;
        g.first_step = Some(next_valid_step(a.step));
        g.events = accepted_timing::Monitor::new(a.now, MIN_EVENT_US, 1000);
        g.campaign_limit = a.campaign;
        g.segment_limit = a.segment;
        g.fault = None;
        true
    }
    #[cfg(feature = "bench-guard-install")]
    pub fn admit(
        now: u32,
        elapsed: u32,
        step: u8,
        feedback: Feedback,
        campaign: u32,
        segment: u32,
        age: u32,
    ) -> Result<Admission<MIN_CYCLE_US, MIN_EVENT_US>, Fault> {
        // Same refusal precedence as with_limits followed by age_initial_feedback.
        if !(1..=1000).contains(&MIN_EVENT_US)
            || !(6..=6000).contains(&MIN_CYCLE_US)
            || MIN_CYCLE_US < 6 * MIN_EVENT_US
        {
            return Err(Fault::CycleTiming);
        }
        if campaign >= 0x8000_0000 || elapsed >= campaign {
            return Err(Fault::CampaignDeadline);
        }
        if segment == 0 || segment >= 0x8000_0000 {
            return Err(Fault::SegmentDeadline);
        }
        if !(1..=6).contains(&step) {
            return Err(Fault::InvalidSeed);
        }
        Self::validate(feedback)?;
        if age > 1000 {
            return Err(Fault::FeedbackStale);
        }
        Ok(Admission {
            now,
            elapsed,
            campaign,
            segment,
            feedback_at: now.wrapping_sub(age),
            step,
        })
    }
    #[cfg(feature = "bench-guard-install")]
    #[inline(always)]
    pub fn install(slot: &mut Option<Self>, a: Admission<MIN_CYCLE_US, MIN_EVENT_US>) {
        *slot = Some(Self {
            start: a.now,
            campaign_elapsed: a.elapsed,
            last_poll: a.now,
            last_feedback: a.feedback_at,
            first_step: Some(next_valid_step(a.step)),
            events: accepted_timing::Monitor::new(a.now, MIN_EVENT_US, 1000),
            fault: None,
            cycle_at: [0; 6],
            cycle_seen: 0,
            campaign_limit: a.campaign,
            segment_limit: a.segment,
            fast_count: 0,
            fast_min: u32::MAX,
            #[cfg(feature = "bench-reentry-next-edge")]
            followed_seed: false,
        });
    }
    pub fn healthy(&self) -> bool {
        self.fault.is_none()
    }
    /// One ordered successor seed before the first driven commutation/event.
    /// Adapter must separately prove outputs off and no COM writer has run.
    /// Poll the EXISTING clocks; do not refresh feedback, tracking or deadlines.
    #[cfg(feature = "bench-reentry-next-edge")]
    pub fn follow_seed(
        &mut self,
        now: u32,
        prior_step: u8,
        next_step: u8,
        no_fault: bool,
        host_abort: bool,
    ) -> Option<Fault> {
        if self.poll(now, no_fault, host_abort).is_some() {
            return self.fault;
        }
        if self.followed_seed
            || !(1..=6).contains(&prior_step)
            || next_valid_step(prior_step) != next_step
            || self.first_step != Some(next_step)
            || self.cycle_seen != 0
        {
            return self.latch(Fault::Tracking);
        }
        self.first_step = Some(next_valid_step(next_step));
        self.followed_seed = true;
        None
    }
    pub fn fast_cycles(&self) -> (u32, u32) {
        (
            self.fast_count,
            if self.fast_count == 0 {
                0
            } else {
                self.fast_min
            },
        )
    }
    pub fn fast_events(&self) -> (u32, u32) {
        self.events.fast_events()
    }
    pub fn event_stale_limit(&self) -> u32 {
        self.events.max_interval()
    }
    pub fn stopped_tracking(&self) -> [u32; 5] {
        let e = self.events.stopped_state();
        [e[0], e[1], e[2], self.last_poll, self.last_feedback]
    }
    /// Fault-only diagnostic. Caller must pass the SAME now/step just supplied
    /// to accepted(), before any later call. The refused cycle leaves at[] intact.
    pub fn refused_cycle(&self, now: u32, step: u8) -> Option<[u32; 4]> {
        if self.fault != Some(Fault::CycleTiming) || !(1..=6).contains(&step) {
            return None;
        }
        let i = (step - 1) as usize;
        if self.cycle_seen & (1 << i) == 0 {
            return None;
        }
        let previous = self.cycle_at[i];
        let delta = now.wrapping_sub(previous);
        if (MIN_CYCLE_US..=6000).contains(&delta) {
            return None;
        }
        Some([step as u32, previous, now, delta])
    }
    pub fn age_initial_feedback(&mut self, age: u32) -> bool {
        if age > 1000 {
            self.latch(Fault::FeedbackStale);
            return false;
        }
        self.last_feedback = self.start.wrapping_sub(age);
        true
    }
    /// Diagnostic timestamp only; cannot refresh or authorize feedback.
    pub fn feedback_timestamp(&self) -> u32 {
        self.last_feedback
    }
    pub fn new(
        now: u32,
        campaign_elapsed: u32,
        seed_step: u8,
        feedback: Feedback,
    ) -> Result<Self, Fault> {
        Self::with_limits(
            now,
            campaign_elapsed,
            seed_step,
            feedback,
            CAMPAIGN_BASE_US,
            20_000,
        )
    }
    #[cfg_attr(feature = "bench-inline-guard", inline(always))]
    pub fn with_limits(
        now: u32,
        campaign_elapsed: u32,
        seed_step: u8,
        feedback: Feedback,
        campaign_limit: u32,
        segment_limit: u32,
    ) -> Result<Self, Fault> {
        if !(1..=1000).contains(&MIN_EVENT_US)
            || !(6..=6000).contains(&MIN_CYCLE_US)
            || MIN_CYCLE_US < 6 * MIN_EVENT_US
        {
            return Err(Fault::CycleTiming);
        }
        if campaign_limit >= 0x8000_0000 || campaign_elapsed >= campaign_limit {
            return Err(Fault::CampaignDeadline);
        }
        if segment_limit == 0 || segment_limit >= 0x8000_0000 {
            return Err(Fault::SegmentDeadline);
        }
        if !(1..=6).contains(&seed_step) {
            return Err(Fault::InvalidSeed);
        }
        Self::validate(feedback)?;
        Ok(Self {
            start: now,
            campaign_elapsed,
            last_poll: now,
            last_feedback: now,
            // At250eHz the reference avg/2 blank is about333us. Alternating
            // edge displacement must not be mistaken for electrical speed.
            first_step: Some(next_valid_step(seed_step)),
            events: accepted_timing::Monitor::new(now, MIN_EVENT_US, 1000),
            fault: None,
            cycle_at: [0; 6],
            cycle_seen: 0,
            campaign_limit,
            segment_limit,
            fast_count: 0,
            fast_min: u32::MAX,
            #[cfg(feature = "bench-reentry-next-edge")]
            followed_seed: false,
        })
    }
    fn validate(f: Feedback) -> Result<(), Fault> {
        validate_feedback(f)
    }
    fn latch(&mut self, f: Fault) -> Option<Fault> {
        self.fault = Some(f);
        self.fault
    }
    /// Must run on an independent bounded timer, including with no BEMF IRQs.
    pub fn poll(&mut self, now: u32, no_fault: bool, host_abort: bool) -> Option<Fault> {
        if self.fault.is_some() {
            return self.fault;
        }
        let elapsed = now.wrapping_sub(self.start);
        let fault = if host_abort {
            Some(Fault::HostAbort)
        } else if !no_fault {
            Some(Fault::Driver)
        } else if elapsed >= self.campaign_limit - self.campaign_elapsed {
            Some(Fault::CampaignDeadline)
        } else if elapsed >= self.segment_limit {
            Some(Fault::SegmentDeadline)
        } else if now.wrapping_sub(self.last_poll) > 200 {
            Some(Fault::TickGap)
        } else if now.wrapping_sub(self.last_feedback) > 1000 {
            Some(Fault::FeedbackStale)
        } else if self.events.poll(now).is_some() {
            Some(Fault::Tracking)
        } else {
            None
        };
        self.last_poll = now;
        if let Some(f) = fault {
            self.latch(f)
        } else {
            None
        }
    }
    /// DMA/publisher delivery with an explicit acquisition-age bound. The
    /// publisher must obtain now and age in this guard's clock domain, and must
    /// not label a cached scan with a new acquisition timestamp. Check delivery
    /// lateness BEFORE applying the acquisition timestamp: a new scan cannot
    /// repair a feedback gap which already exceeded the unchanged1ms limit.
    pub fn feedback_aged(&mut self, now: u32, age_us: u32, f: Feedback) -> Option<Fault> {
        if self.fault.is_some() {
            return self.fault;
        }
        if age_us > 1000 || now.wrapping_sub(self.last_feedback) > 1000 {
            return self.latch(Fault::FeedbackStale);
        }
        self.feedback(now.wrapping_sub(age_us), f)
    }
    /// Timestamp must be acquisition time, not dump time. Do not erase a gap
    /// with fresh feedback after the deadline has already been missed.
    pub fn feedback(&mut self, now: u32, f: Feedback) -> Option<Fault> {
        if self.fault.is_some() {
            return self.fault;
        }
        if now.wrapping_sub(self.last_feedback) > 1000 {
            return self.latch(Fault::FeedbackStale);
        }
        if let Err(reason) = Self::validate(f) {
            return self.latch(reason);
        }
        self.last_feedback = now;
        None
    }
    /// Only actual EV_ACC enters here; bootstrap COM is not an accepted event.
    pub fn accepted(&mut self, now: u32, step: u8) -> Option<Fault> {
        if self.fault.is_some() {
            return self.fault;
        }
        if self
            .first_step
            .take()
            .is_some_and(|expected| step != expected)
            || self.events.event_policy::<REPORT_FAST>(now, step).is_some()
        {
            return self.latch(Fault::Tracking);
        }
        let i = (step - 1) as usize;
        let bit = 1 << i;
        if self.cycle_seen & bit != 0 {
            let delta = now.wrapping_sub(self.cycle_at[i]);
            // Only the fast side is advisory in the explicitly selected policy.
            // The timestamp still advances on each accepted event, never hides
            // a short cycle behind an earlier observation or resets tracking.
            if delta > 6000 || (delta < MIN_CYCLE_US && !REPORT_FAST) {
                return self.latch(Fault::CycleTiming);
            }
            #[cfg(not(feature = "bench-lean-irq"))]
            if REPORT_FAST && delta < MIN_CYCLE_US {
                self.fast_count = self.fast_count.saturating_add(1);
                self.fast_min = self.fast_min.min(delta);
            }
        }
        self.cycle_at[i] = now;
        self.cycle_seen |= bit;
        None
    }
    /// Use the controller's half-microsecond interval estimate to bound a
    /// missing accepted event. Three expected event periods tolerate an
    /// isolated miss; the independent 100 us poll supplies the final latency.
    /// The monitor only tightens, so a developing slowdown cannot move its own
    /// deadline outward. The historical 1 ms ceiling remains at low speed.
    #[cfg(feature = "bench-speed-event-watch")]
    pub fn accepted_speed_scaled(
        &mut self,
        now: u32,
        step: u8,
        reference_half_us: u32,
    ) -> Option<Fault> {
        let fault = self.accepted(now, step);
        // Zero/small values are uninitialized or outside the intended <=100%
        // no-load regime. They cannot establish a shorter deadline.
        if fault.is_none() && reference_half_us >= 64 {
            let _ = self
                .events
                .tighten_max_interval(speed_event_limit_us(reference_half_us));
        }
        fault
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "bench-speed-event-watch")]
    #[test]
    fn speed_event_watch_tightens_but_never_tracks_a_slowdown_outward() {
        type G = RunGuard<2223, 100, true>;
        let mut g = G::with_limits(0, 0, 1, sample(), 100_000, 20_000).unwrap();
        assert_eq!(g.accepted_speed_scaled(100, 2, 0), None);
        assert_eq!(g.event_stale_limit(), 1000);
        assert_eq!(g.accepted_speed_scaled(200, 3, 600), None);
        assert_eq!(g.event_stale_limit(), 900);
        assert_eq!(g.poll(200, true, false), None);
        assert_eq!(g.poll(300, true, false), None);
        assert_eq!(g.accepted_speed_scaled(400, 4, 200), None);
        assert_eq!(g.event_stale_limit(), 300);
        assert_eq!(g.poll(500, true, false), None);
        assert_eq!(g.poll(600, true, false), None);
        assert_eq!(g.accepted_speed_scaled(650, 5, 900), None);
        assert_eq!(g.event_stale_limit(), 300);
        assert_eq!(g.poll(750, true, false), None);
        assert_eq!(g.poll(850, true, false), None);
        assert_eq!(g.poll(950, true, false), None);
        assert_eq!(g.poll(951, true, false), Some(Fault::Tracking));
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn following_seed_changes_only_expected_sector_not_clock_authority() {
        type G = RunGuard<2223, 100, true>;
        for start in [0, u32::MAX - 300] {
            for prior in 1..=6 {
                let mut g = G::with_limits(start, 1000, prior, sample(), 100000, 20000).unwrap();
                assert!(g.age_initial_feedback(200));
                for dt in [100, 200, 300, 400] {
                    assert_eq!(g.poll(start.wrapping_add(dt), true, false), None);
                }
                let original = (
                    g.start,
                    g.campaign_elapsed,
                    g.last_feedback,
                    g.campaign_limit,
                    g.segment_limit,
                );
                let next = next_valid_step(prior);
                assert_eq!(
                    g.follow_seed(start.wrapping_add(500), prior, next, true, false),
                    None
                );
                assert_eq!(
                    (
                        g.start,
                        g.campaign_elapsed,
                        g.last_feedback,
                        g.campaign_limit,
                        g.segment_limit
                    ),
                    original
                );
                assert_eq!(
                    g.accepted(start.wrapping_add(600), next_valid_step(next)),
                    None
                );
            }
        }
    }
    #[cfg(feature = "bench-reentry-next-edge")]
    #[test]
    fn follow_does_not_repair_tracking_feedback_deadline_or_second_follow() {
        type G = RunGuard<2223, 100, true>;
        let make = || G::with_limits(0, 1000, 1, sample(), 100000, 20000).unwrap();
        let mut g = make();
        assert_eq!(g.follow_seed(100, 1, 2, true, false), None);
        assert_eq!(g.follow_seed(200, 2, 3, true, false), Some(Fault::Tracking));
        for (no_fault, abort, why) in [
            (false, false, Fault::Driver),
            (true, true, Fault::HostAbort),
        ] {
            assert_eq!(make().follow_seed(100, 1, 2, no_fault, abort), Some(why));
        }
        let mut g = make();
        g.last_poll = 1001;
        g.last_feedback = 1001;
        assert_eq!(
            g.follow_seed(1001, 1, 2, true, false),
            Some(Fault::Tracking)
        );
        let mut g = make();
        g.last_poll = 1001;
        assert_eq!(
            g.follow_seed(1001, 1, 2, true, false),
            Some(Fault::FeedbackStale)
        );
        let mut g = make();
        g.segment_limit = 100;
        assert_eq!(
            g.follow_seed(100, 1, 2, true, false),
            Some(Fault::SegmentDeadline)
        );
        assert_eq!(
            make().follow_seed(201, 1, 2, true, false),
            Some(Fault::TickGap)
        );
        assert_eq!(
            make().follow_seed(100, 1, 3, true, false),
            Some(Fault::Tracking)
        );
    }
    #[cfg(feature = "bench-reentry-guard-stage")]
    #[test]
    fn staged_install_matches_fresh_and_never_refreshes_admission() {
        type G = RunGuard<2223, 100, true>;
        for now in [0, u32::MAX - 50] {
            for step in 1..=6 {
                for age in [0, 500, 1000] {
                    let mut expected = None;
                    G::install(
                        &mut expected,
                        G::admit(now, 1234, step, sample(), 100000, 20000, age).unwrap(),
                    );
                    let mut staged =
                        Some(G::with_limits(50, 8000, 6, sample(), 100000, 30000).unwrap());
                    let old = staged.as_mut().unwrap();
                    old.cycle_at = [u32::MAX; 6];
                    old.cycle_seen = 63;
                    old.fast_count = 123;
                    old.fast_min = 17;
                    old.fault = Some(Fault::Current);
                    #[cfg(feature = "bench-reentry-next-edge")]
                    {
                        old.followed_seed = true;
                    }
                    G::stage_install(&mut staged);
                    assert!(!staged.as_ref().unwrap().healthy());
                    assert!(G::install_staged(
                        &mut staged,
                        G::admit(now, 1234, step, sample(), 100000, 20000, age).unwrap()
                    ));
                    let (a, b) = (expected.as_mut().unwrap(), staged.as_mut().unwrap());
                    #[cfg(feature = "bench-reentry-next-edge")]
                    {
                        assert!(!a.followed_seed && !b.followed_seed);
                    }
                    assert_eq!(
                        (
                            a.start,
                            a.campaign_elapsed,
                            a.last_poll,
                            a.last_feedback,
                            a.first_step
                        ),
                        (
                            b.start,
                            b.campaign_elapsed,
                            b.last_poll,
                            b.last_feedback,
                            b.first_step
                        )
                    );
                    assert_eq!(
                        (
                            a.cycle_at,
                            a.cycle_seen,
                            a.campaign_limit,
                            a.segment_limit,
                            a.fast_count,
                            a.fast_min
                        ),
                        (
                            b.cycle_at,
                            b.cycle_seen,
                            b.campaign_limit,
                            b.segment_limit,
                            b.fast_count,
                            b.fast_min
                        )
                    );
                    for dt in (0..25000u32).step_by(100) {
                        let t = now.wrapping_add(dt);
                        assert_eq!(
                            a.feedback_aged(t, age, sample()),
                            b.feedback_aged(t, age, sample())
                        );
                        let sector = ((u32::from(step) + dt / 100) % 6 + 1) as u8;
                        assert_eq!(a.accepted(t, sector), b.accepted(t, sector));
                        assert_eq!(a.poll(t, true, false), b.poll(t, true, false));
                        assert_eq!(a.healthy(), b.healthy());
                    }
                }
            }
        }
    }
    #[cfg(feature = "bench-reentry-guard-stage")]
    #[test]
    fn missing_live_and_consumed_staging_are_refused() {
        type G = RunGuard<2223, 100, true>;
        let admission = || G::admit(0, 1000, 1, sample(), 100000, 20000, 0).unwrap();
        let mut slot = None;
        assert!(!G::install_staged(&mut slot, admission()));
        G::install(&mut slot, admission());
        assert!(!G::install_staged(&mut slot, admission()));
        G::stage_install(&mut slot);
        assert_eq!(
            slot.as_mut().unwrap().poll(50000, true, false),
            Some(Fault::InvalidSeed)
        );
        assert!(!slot.as_ref().unwrap().healthy());
        assert!(G::install_staged(&mut slot, admission()));
        assert!(!G::install_staged(&mut slot, admission()));
        assert_eq!(slot.as_ref().unwrap().feedback_timestamp(), 0);
    }
    #[test]
    fn report_policy_retains_slow_side_and_exact_fast_boundary() {
        type G = RunGuard<2223, 238, true>;
        for delta in [2222, 2223, 2224, 6000, 6001] {
            let mut g = G::with_limits(0, 0, 1, sample(), 100000, 50000).unwrap();
            // Isolate cycle predicate from independent event-age checks.
            g.cycle_seen = 2;
            g.cycle_at[1] = 100u32.wrapping_sub(delta);
            assert_eq!(
                g.accepted(100, 2),
                if delta > 6000 {
                    Some(Fault::CycleTiming)
                } else {
                    None
                }
            );
            assert_eq!(
                g.fast_cycles(),
                if delta < 2223 { (1, delta) } else { (0, 0) }
            );
        }
    }
    fn sample() -> Feedback {
        Feedback {
            phase: [2048; 3],
            bus_mv: 11800,
            vref: 1500,
        }
    }
    #[cfg(feature = "bench-guard-install")]
    #[test]
    fn admission_preserves_refusal_precedence_and_age() {
        type G = RunGuard<3125, 260>;
        let bad_current = Feedback {
            phase: [847, 2048, 2048],
            ..sample()
        };
        let bad_bus = Feedback {
            bus_mv: 8399,
            ..sample()
        };
        let bad_both = Feedback {
            bus_mv: 8399,
            ..bad_current
        };
        for now in [0, u32::MAX - 50] {
            for step in 0..=7 {
                for age in [0, 1000, 1001] {
                    for feedback in [sample(), bad_current, bad_bus, bad_both] {
                        for (elapsed, campaign) in [(0, 10000), (10000, 10000), (0, 0x80000000)] {
                            for segment in [0, 5000, 0x80000000] {
                                let old =
                                    G::with_limits(now, elapsed, step, feedback, campaign, segment)
                                        .and_then(|mut g| {
                                            if g.age_initial_feedback(age) {
                                                Ok(g)
                                            } else {
                                                Err(Fault::FeedbackStale)
                                            }
                                        });
                                let new =
                                    G::admit(now, elapsed, step, feedback, campaign, segment, age);
                                assert_eq!(old.as_ref().err(), new.as_ref().err());
                                if let (Ok(old), Ok(a)) = (old, new) {
                                    let mut slot = None;
                                    G::install(&mut slot, a);
                                    let mut new = slot.unwrap();
                                    assert_eq!(new.feedback_timestamp(), old.feedback_timestamp());
                                    let mut old = old;
                                    for dt in [0, 100, 201, 1001] {
                                        assert_eq!(
                                            old.poll(now.wrapping_add(dt), true, false),
                                            new.poll(now.wrapping_add(dt), true, false)
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(
            RunGuard::<5, 260>::admit(0, 0, 1, sample(), 1000, 500, 0).err(),
            Some(Fault::CycleTiming)
        );
    }
    #[cfg(feature = "bench-guard-install")]
    #[test]
    fn installed_guard_matches_events_and_replaces_stopped_history() {
        type G = RunGuard<3125, 260>;
        let mut slot = Some(G::new(0, 0, 1, sample()).unwrap());
        assert_eq!(
            slot.as_mut().unwrap().poll(1001, true, false),
            Some(Fault::TickGap)
        );
        G::install(
            &mut slot,
            G::admit(10, 200, 3, sample(), 100000, 50000, 25).unwrap(),
        );
        let mut old = G::with_limits(10, 200, 3, sample(), 100000, 50000).unwrap();
        old.age_initial_feedback(25);
        let new = slot.as_mut().unwrap();
        for n in 1..=24 {
            let t = 10 + n * 550;
            let step = ((n + 2) % 6 + 1) as u8;
            assert_eq!(old.accepted(t, step), new.accepted(t, step));
        }
        assert_eq!(old.accepted(13211, 4), new.accepted(13211, 4));
        assert_eq!(old.healthy(), new.healthy());
    }
    #[test]
    fn bounded_successor_only_receives_admitted_sectors() {
        type G = RunGuard<3031, 252>;
        for step in 0..=u8::MAX {
            let old = G::with_limits(0, 200, step, sample(), 100000, 50000);
            if (1..=6).contains(&step) {
                assert_eq!(old.unwrap().first_step, Some(step % 6 + 1));
            } else {
                assert_eq!(old.err(), Some(Fault::InvalidSeed));
            }
            #[cfg(feature = "bench-guard-install")]
            {
                let admitted = G::admit(0, 200, step, sample(), 100000, 50000, 0);
                if (1..=6).contains(&step) {
                    let mut slot = None;
                    G::install(&mut slot, admitted.unwrap());
                    assert_eq!(slot.unwrap().first_step, Some(step % 6 + 1));
                } else {
                    assert_eq!(admitted.err(), Some(Fault::InvalidSeed));
                }
            }
        }
    }
    #[test]
    fn experimental_running_envelope_does_not_change_default_or_add_ram() {
        // ~300eHz synthetic events, NOT a300eHz bench qualification.
        type Faster = RunGuard<3333, 277>;
        assert_eq!(
            core::mem::size_of::<Guard>(),
            core::mem::size_of::<Faster>()
        );
        let mut old = Guard::new(0, 0, 1, sample()).unwrap();
        let mut next = Faster::new(0, 0, 1, sample()).unwrap();
        for n in 1..=18 {
            let step = (n % 6 + 1) as u8;
            assert_eq!(next.accepted(n * 556, step), None);
            assert_eq!(
                old.accepted(n * 556, step),
                if n < 7 {
                    None
                } else {
                    Some(Fault::CycleTiming)
                }
            );
        }
    }
    #[test]
    fn refused_cycle_preserves_exact_guard_times_including_wrap() {
        for origin in [0, u32::MAX - 2000] {
            let mut a = RunGuard::<3333, 277>::new(origin, 0, 1, sample()).unwrap();
            for n in 1..=6 {
                let at = origin.wrapping_add(n * 555);
                let step = (n % 6 + 1) as u8;
                assert_eq!(a.accepted(at, step), None);
                assert_eq!(a.refused_cycle(at, step), None);
            }
            let at = origin.wrapping_add(7 * 555);
            assert_eq!(a.accepted(at, 2), Some(Fault::CycleTiming));
            assert_eq!(
                a.refused_cycle(at, 2),
                Some([2, origin.wrapping_add(555), at, 3330])
            );
            assert_eq!(a.refused_cycle(at, 0), None);
            assert_eq!(a.refused_cycle(at, 7), None);
        }
    }
    #[test]
    fn faster_profile_preserves_current_missing_event_and_overspeed_refusal() {
        type Faster = RunGuard<3333, 277>;
        let mut a = Faster::new(0, 0, 1, sample()).unwrap();
        for t in (100..=1000).step_by(100) {
            assert_eq!(a.feedback(t, sample()), None);
            assert_eq!(a.poll(t, true, false), None);
        }
        assert_eq!(a.poll(1001, true, false), Some(Fault::Tracking));
        let mut a = Faster::new(0, 0, 1, sample()).unwrap();
        let mut bad = sample();
        bad.phase[2] = 3249;
        assert_eq!(a.feedback(10, bad), Some(Fault::Current));
        let mut a = Faster::new(0, 0, 1, sample()).unwrap();
        for n in 1..=6 {
            assert_eq!(a.accepted(n * 555, (n % 6 + 1) as u8), None);
        }
        assert_eq!(a.accepted(7 * 555, 2), Some(Fault::CycleTiming));
        assert!(RunGuard::<0, 277>::new(0, 0, 1, sample()).is_err());
        assert!(RunGuard::<3333, 1001>::new(0, 0, 1, sample()).is_err());
        assert!(RunGuard::<3333, 600>::new(0, 0, 1, sample()).is_err());
    }
    #[test]
    fn range310_still_rejects_overspeed_current_and_missing_events() {
        type Next = RunGuard<3226, 268>;
        for spacing in [537, 538] {
            let mut g = Next::new(0, 0, 1, sample()).unwrap();
            for n in 1..=6 {
                assert_eq!(g.accepted(n * spacing, (n % 6 + 1) as u8), None);
            }
            assert_eq!(
                g.accepted(7 * spacing, 2),
                if spacing == 537 {
                    Some(Fault::CycleTiming)
                } else {
                    None
                }
            );
        }
        let mut g = Next::new(0, 0, 1, sample()).unwrap();
        for t in (100..=1000).step_by(100) {
            assert_eq!(g.feedback(t, sample()), None);
            assert_eq!(g.poll(t, true, false), None);
        }
        assert_eq!(g.poll(1001, true, false), Some(Fault::Tracking));
        let mut bad = sample();
        bad.phase[0] = 3249;
        assert!(matches!(Next::new(0, 0, 1, bad), Err(Fault::Current)));
        let mut g = Next::new(0, 0, 1, sample()).unwrap();
        assert!(!g.age_initial_feedback(1001));
    }
    #[test]
    fn range320_preserves_current_age_and_cycle_checks() {
        type Next = RunGuard<3125, 260>;
        for spacing in [520, 521] {
            let mut g = Next::new(0, 0, 1, sample()).unwrap();
            for n in 1..=6 {
                assert_eq!(g.accepted(n * spacing, (n % 6 + 1) as u8), None);
            }
            assert_eq!(
                g.accepted(7 * spacing, 2),
                if spacing == 520 {
                    Some(Fault::CycleTiming)
                } else {
                    None
                }
            );
        }
        let mut g = Next::new(0, 0, 1, sample()).unwrap();
        for t in (100..=1000).step_by(100) {
            assert_eq!(g.feedback(t, sample()), None);
            assert_eq!(g.poll(t, true, false), None);
        }
        assert_eq!(g.poll(1001, true, false), Some(Fault::Tracking));
        let mut bad = sample();
        bad.phase[0] = 3249;
        assert!(matches!(Next::new(0, 0, 1, bad), Err(Fault::Current)));
        let mut g = Next::new(0, 0, 1, sample()).unwrap();
        assert!(!g.age_initial_feedback(1001));
    }
    #[test]
    fn range330_preserves_current_age_and_cycle_checks() {
        type Next = RunGuard<3031, 252>;
        for spacing in [505, 506] {
            let mut g = Next::new(0, 0, 1, sample()).unwrap();
            for n in 1..=6 {
                assert_eq!(g.accepted(n * spacing, (n % 6 + 1) as u8), None);
            }
            assert_eq!(
                g.accepted(7 * spacing, 2),
                if spacing == 505 {
                    Some(Fault::CycleTiming)
                } else {
                    None
                }
            );
        }
        let mut g = Next::new(0, 0, 1, sample()).unwrap();
        for t in (100..=1000).step_by(100) {
            assert_eq!(g.feedback(t, sample()), None);
            assert_eq!(g.poll(t, true, false), None);
        }
        assert_eq!(g.poll(1001, true, false), Some(Fault::Tracking));
        let mut bad = sample();
        bad.phase[0] = 3249;
        assert!(matches!(Next::new(0, 0, 1, bad), Err(Fault::Current)));
        let mut g = Next::new(0, 0, 1, sample()).unwrap();
        assert!(!g.age_initial_feedback(1001));
    }
    #[test]
    fn recovery_retains_original_deadline_after_long_run_and_wrap() {
        let (mut budget, first) = SessionBudget::initial(4_700_000, 60_000_000).unwrap();
        assert_eq!(
            first,
            Limits {
                elapsed: 4_700_200,
                campaign: 65_000_000,
                segment: 60_000_000
            }
        );
        //45s into the run plus12ms disabled wake/qualification/setup.
        let limits = budget.reentry(49_712_000, Fault::Tracking).unwrap();
        assert_eq!(
            limits,
            Limits {
                elapsed: 49_712_200,
                campaign: 65_000_000,
                segment: 14_987_800
            }
        );
        let origin = u32::MAX - 1000;
        let mut guard = Guard::with_limits(
            origin,
            limits.elapsed,
            1,
            sample(),
            limits.campaign,
            limits.segment,
        )
        .unwrap();
        for t in (100..limits.segment).step_by(100) {
            let now = origin.wrapping_add(t);
            assert_eq!(guard.feedback(now, sample()), None);
            if t % 800 == 0 {
                assert_eq!(guard.accepted(now, (((t / 800) % 6) + 1) as u8), None);
            }
            assert_eq!(guard.poll(now, true, false), None);
        }
        assert_eq!(
            guard.poll(origin.wrapping_add(limits.segment), true, false),
            Some(Fault::SegmentDeadline)
        );
        assert_eq!(
            budget.reentry(50_000_000, Fault::Tracking),
            Err(ReentryRefusal::AlreadyAttempted)
        );
    }
    #[test]
    fn driven_reserve_shortens_drive_without_extending_original_deadline() {
        let (mut old, _) = SessionBudget::initial(4_712_086, 10_000_000).unwrap();
        let (mut driven, _) = SessionBudget::initial(4_712_086, 10_000_000).unwrap();
        let legacy = old.reentry(6_723_741, Fault::Tracking).unwrap();
        let limits = driven
            .reentry_reserved::<300>(6_723_741, Fault::Tracking)
            .unwrap();
        assert_eq!(limits.segment + 100, legacy.segment);
        assert_eq!(6_723_741 + 300 + limits.segment, 14_712_086);
        assert_eq!(limits.campaign, legacy.campaign);
        assert_eq!(
            driven.reentry_reserved::<300>(6_724_000, Fault::Tracking),
            Err(ReentryRefusal::AlreadyAttempted)
        );
        let (mut invalid, _) = SessionBudget::initial(0, 100_000).unwrap();
        assert_eq!(
            invalid.reentry_reserved::<199>(10_000, Fault::Tracking),
            Err(ReentryRefusal::Deadline)
        );
        let (mut late, _) = SessionBudget::initial(0, 100_000).unwrap();
        assert_eq!(
            late.reentry_reserved::<300>(79_701, Fault::Tracking),
            Err(ReentryRefusal::Deadline)
        );
    }
    #[test]
    fn reentry_cannot_retry_electrical_host_or_exhausted_campaign() {
        for fault in [
            Fault::Current,
            Fault::Bus,
            Fault::Driver,
            Fault::HostAbort,
            Fault::AdcTimeout,
            Fault::CycleTiming,
            Fault::FeedbackStale,
            Fault::TickGap,
        ] {
            let (mut b, _) = SessionBudget::initial(4_700_000, 10_000_000).unwrap();
            assert_eq!(b.reentry(6_712_000, fault), Err(ReentryRefusal::WrongFault));
            assert_eq!(
                b.reentry(6_713_000, Fault::Tracking),
                Err(ReentryRefusal::AlreadyAttempted)
            );
        }
        for elapsed in [0, 4_699_999, 14_680_000, 14_700_000, u32::MAX] {
            let (mut b, _) = SessionBudget::initial(4_700_000, 10_000_000).unwrap();
            assert_eq!(
                b.reentry(elapsed, Fault::Tracking),
                Err(ReentryRefusal::Deadline)
            );
        }
        assert!(SessionBudget::initial(5_000_000, 10_000_000).is_none());
        assert!(SessionBudget::initial(0, 600_000_001).is_none());
    }
    #[test]
    fn shutdown_margin_is_inside_original_campaign_budget() {
        assert_eq!(reserved_elapsed(4_700_000), Some(4_700_200));
        assert_eq!(reserved_elapsed(4_999_799), Some(4_999_999));
        assert_eq!(reserved_elapsed(4_999_800), None);
        assert_eq!(reserved_elapsed(u32::MAX), None);
    }
    fn g() -> Guard {
        Guard::new(0, 4_700_000, 1, sample()).unwrap()
    }
    #[test]
    fn two_minutes_preserves_guards_and_exact_deadline() {
        let mut a =
            Guard::with_limits(0, 4_700_200, 1, sample(), 125_000_000, 120_000_000).unwrap();
        let mut step = 2;
        for t in (100..120_000_000).step_by(100) {
            assert_eq!(a.feedback(t, sample()), None);
            if t % 800 == 0 {
                assert_eq!(a.accepted(t, step), None);
                step = step % 6 + 1;
            }
            assert_eq!(a.poll(t, true, false), None);
        }
        assert_eq!(
            a.poll(120_000_000, true, false),
            Some(Fault::SegmentDeadline)
        );
        assert_eq!(a.accepted(120_000_001, step), Some(Fault::SegmentDeadline));
    }
    #[test]
    fn invalid_long_limits_and_shorter_campaign_are_not_bypassed() {
        assert!(Guard::with_limits(0, 0, 1, sample(), 1000, 0).is_err());
        assert!(Guard::with_limits(0, 0, 1, sample(), 0x8000_0000, 100).is_err());
        let mut a = Guard::with_limits(0, 900, 1, sample(), 1000, 120_000_000).unwrap();
        assert_eq!(a.poll(100, true, false), Some(Fault::CampaignDeadline));
    }
    #[test]
    fn measured_alternating_events_preserve_full_cycle() {
        let mut a = Guard::new(0, 4_700_000, 5, sample()).unwrap();
        for (t, s) in [
            (752, 6),
            (1677, 1),
            (2402, 2),
            (3315, 3),
            (4029, 4),
            (4943, 5),
            (5559, 6),
        ] {
            assert_eq!(a.accepted(t, s), None);
        }
    }
    #[test]
    fn accelerated_train_cannot_hide_behind_legal_individual_gaps() {
        let mut a = g();
        for n in 0..6 {
            assert_eq!(a.accepted(500 + n * 500, ((n + 1) % 6 + 1) as u8), None);
        }
        assert_eq!(a.accepted(3500, 2), Some(Fault::CycleTiming));
        assert_eq!(a.accepted(4000, 3), Some(Fault::CycleTiming));
    }
    #[test]
    fn cycle_boundaries_and_wrap() {
        for period in [4000u32, 6000] {
            let start = u32::MAX - 100;
            let mut a = Guard::new(start, 4_700_000, 1, sample()).unwrap();
            for n in 0..=6 {
                assert_eq!(
                    a.accepted(start.wrapping_add(n * period / 6), ((n + 1) % 6 + 1) as u8),
                    None
                );
            }
        }
        let mut a = g();
        assert_eq!(a.accepted(500, 2), None);
        assert_eq!(a.accepted(832, 3), Some(Fault::Tracking));
    }
    #[test]
    fn acquired_baseline_age_is_not_reset_at_guard_start() {
        let mut a = g();
        assert!(a.age_initial_feedback(801));
        assert_eq!(a.poll(200, true, false), Some(Fault::FeedbackStale));
        let mut a = g();
        assert!(a.age_initial_feedback(801));
        assert_eq!(a.feedback(199, sample()), None);
        assert_eq!(a.poll(200, true, false), None);
        let mut a = g();
        assert!(!a.age_initial_feedback(1001));
        assert!(!a.healthy());
    }
    #[test]
    fn fresh_first_dma_frame_cannot_repair_expired_initial_feedback() {
        let mut a = g();
        assert!(a.age_initial_feedback(650));
        assert_eq!(a.feedback_timestamp(), 0u32.wrapping_sub(650));
        // Frame age167us is acceptable, but previous feedback age1059us is not.
        assert_eq!(
            a.feedback_aged(409, 167, sample()),
            Some(Fault::FeedbackStale)
        );
        assert_eq!(a.feedback_timestamp(), 0u32.wrapping_sub(650));
        let mut b = g();
        assert!(b.age_initial_feedback(650));
        assert_eq!(b.feedback_aged(340, 98, sample()), None);
        assert_eq!(b.feedback_timestamp(), 242);
    }
    #[test]
    fn e643_initial_age_requires_real_frame_within_remaining_slack() {
        let mut late = g();
        assert!(late.age_initial_feedback(893));
        assert_eq!(late.poll(100, true, false), None);
        assert_eq!(
            late.feedback_aged(108, 20, sample()),
            Some(Fault::FeedbackStale)
        );
        assert_eq!(late.feedback_timestamp(), 0u32.wrapping_sub(893));
        let mut early = g();
        assert!(early.age_initial_feedback(893));
        assert_eq!(early.feedback_aged(80, 30, sample()), None);
        assert_eq!(early.feedback_timestamp(), 50); // actual acquisition, not80
        assert_eq!(early.poll(100, true, false), None);
        assert_eq!(early.poll(209, true, false), None);
    }
    #[test]
    fn fresh_data_cannot_hide_feedback_or_poll_gap() {
        let mut a = g();
        assert_eq!(a.feedback(1001, sample()), Some(Fault::FeedbackStale));
        assert_eq!(a.poll(1002, true, false), Some(Fault::FeedbackStale));
        let mut a = g();
        assert_eq!(a.poll(201, true, false), Some(Fault::TickGap));
    }
    #[test]
    fn cached_dma_frame_does_not_refresh_acquisition_age() {
        let mut a = g();
        assert_eq!(a.feedback_aged(150, 50, sample()), None);
        for now in (200..=1100).step_by(100) {
            assert_eq!(a.feedback_aged(now, now - 100, sample()), None);
            assert_eq!(a.last_feedback, 100);
        }
        assert_eq!(
            a.feedback_aged(1101, 1001, sample()),
            Some(Fault::FeedbackStale)
        );
    }
    #[test]
    fn delayed_or_reordered_dma_delivery_cannot_repair_a_gap() {
        let mut a = g();
        assert_eq!(
            a.feedback_aged(1001, 1, sample()),
            Some(Fault::FeedbackStale)
        );
        let mut a = g();
        assert_eq!(a.feedback_aged(200, 50, sample()), None);
        assert_eq!(
            a.feedback_aged(210, 100, sample()),
            Some(Fault::FeedbackStale)
        );
        assert_eq!(
            a.feedback_aged(211, 0, sample()),
            Some(Fault::FeedbackStale)
        );
    }
    #[test]
    fn aged_dma_feedback_keeps_electrical_guards_and_clock_wrap() {
        let mut a = g();
        a.start = u32::MAX - 100;
        a.last_feedback = a.start;
        assert_eq!(a.feedback_aged(50, 25, sample()), None);
        assert_eq!(a.last_feedback, 25);
        let mut bad = sample();
        bad.phase[0] = 4095;
        assert_eq!(a.feedback_aged(60, 20, bad), Some(Fault::Current));
    }
    #[test]
    fn no_first_edge_and_wrong_first_sector_are_faults() {
        let mut a = g();
        for t in (100..=1000).step_by(100) {
            assert_eq!(a.feedback(t, sample()), None);
            assert_eq!(a.poll(t, true, false), None);
        }
        assert_eq!(a.poll(1100, true, false), Some(Fault::Tracking));
        let mut a = g();
        assert_eq!(a.accepted(700, 1), Some(Fault::Tracking));
        let mut a = g();
        assert_eq!(a.accepted(700, 2), None);
        assert_eq!(a.accepted(800, 3), Some(Fault::Tracking));
    }
    #[test]
    fn prior_open_loop_time_is_not_reset_by_handoff() {
        let mut a = Guard::new(100, 4_999_900, 1, sample()).unwrap();
        assert_eq!(a.poll(200, true, false), Some(Fault::CampaignDeadline));
        let mut a = Guard::new(u32::MAX - 49, 4_999_900, 1, sample()).unwrap();
        assert_eq!(a.poll(50, true, false), Some(Fault::CampaignDeadline));
    }
    #[test]
    fn segment_is_bounded_even_with_continuous_valid_events() {
        let mut a = g();
        let mut step = 2;
        for t in (100..20_000).step_by(100) {
            a.feedback(t, sample());
            if t % 800 == 0 {
                assert_eq!(a.accepted(t, step), None);
                step = step % 6 + 1;
            }
            assert_eq!(a.poll(t, true, false), None);
        }
        assert_eq!(a.poll(20_000, true, false), Some(Fault::SegmentDeadline));
    }
    #[test]
    fn pulse_bus_driver_and_host_guards_latch() {
        for f in [
            Feedback {
                phase: [0, 2048, 2048],
                ..sample()
            },
            Feedback {
                phase: [3249, 2048, 2048],
                ..sample()
            },
        ] {
            let mut a = g();
            assert_eq!(a.feedback(1, f), Some(Fault::Current));
        }
        for f in [
            Feedback {
                bus_mv: 8399,
                ..sample()
            },
            Feedback {
                vref: 0,
                ..sample()
            },
        ] {
            let mut a = g();
            assert_eq!(a.feedback(1, f), Some(Fault::Bus));
        }
        let mut a = g();
        assert_eq!(a.poll(1, false, false), Some(Fault::Driver));
        assert_eq!(a.poll(2, true, false), Some(Fault::Driver));
        let mut a = g();
        assert_eq!(a.poll(1, true, true), Some(Fault::HostAbort));
    }
}
