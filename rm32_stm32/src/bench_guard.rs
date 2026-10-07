//! Bench safety guard — absolute vbat-sag and overcurrent kill, latched.
//!
//! Mirrors the minz am32_clone bench-kill thresholds that kept that effort
//! from burning the bench (minz/core/src/am32_isr.rs, retuned after the
//! 2026-07-24/25 clone-vs-AM32 study): the guard's target is PSU collapse,
//! battery death, and stalled-winding heating — events that go DEEP and
//! STAY. Transients shallower than the debounce (throttle-slam inrush sag)
//! ride through.
//!
//! This is firmware-side only (not part of the portable core, not linked
//! into the vector harness) and runs unconditionally in the main loop.
//! Consequence on trip mirrors the LVC path (rm32/src/main_state.rs):
//! `IsrAction::AllOff` + `MotorEvent::Disarm`, re-asserted every loop while
//! latched. Only a reset re-arms — a kill is evidence, not a hiccup.

/// Absolute vbat floor in millivolts (~5.5 V). Below the 2S operating
/// range of this bench; a healthy bus sits at ~8.1 V.
const VBAT_FLOOR_MV: u16 = 5500;
/// vbat must sit below the floor this long before the kill fires.
const VBAT_DEBOUNCE_MS: u32 = 10;
/// Sustained-current kill threshold (mA). minz: 205 raw counts avg
/// ≈ 165 mV / 30 mV-per-A ≈ 5.5 A.
const OC_KILL_MA: i16 = 5500;
/// Current must exceed the threshold this long (minz used an ~85 ms
/// windowed average; measurements here are already median-filtered).
const OC_DEBOUNCE_MS: u32 = 85;
/// Over-voltage kill: regen pumping into a source-only bench PSU drives
/// the 8.1 V rail to 9.5-11.9 V (measured 07-25, big-picture traces).
/// AllOff is safe here: with all FETs off the synchronous rectification
/// stops and BEMF at bench speeds stays below the rail. 5 ms debounce
/// rides out ADC blips; sustained pumping trips fast.
// 10.8 V / 60 ms: brief regen spikes at fall boundaries reach ~10-11 V
// for a few ms and must ride through (10.2 V / 5 ms killed 3/4 climbs);
// sustained pumping (the 11.9 V events lasted 100s of ms) still trips.
const OV_KILL_MV: u16 = 10_800;
const OV_DEBOUNCE_MS: u32 = 60;

/// Battery-source profile (3S pack). OC 15 A (slam accel measured
/// ~10 A average in the battB_ sessions — a real operating point, 5x
/// stall margin), OV 14.0 V (pack rest tops ~12.6 V; regen charges the
/// pack instead of pumping, so OV only guards a genuinely wrong source
/// state). Selected automatically by the rest-voltage peak: >10.5 V
/// rest = battery.
///
/// Vbat floor: the original 8.47 V came from battB_ sessions that
/// never exceeded ~70 % throttle. First full-envelope battery runs
/// (2026-08-08 re-qual) measured healthy 3S packs at 8.4-8.5 V STEADY
/// at 100 % and 7.24-7.39 V transient at slam inrush — the old floor
/// sat inside the operating band and killed both re-qual halves
/// mid-test. 6.8 V / 150 ms sits below healthy inrush with margin;
/// a genuinely dying pack collapses under load and STAYS there, so
/// the longer debounce still catches it while accel transients ride
/// through.
const B_VBAT_FLOOR_MV: u16 = 6_800;
const B_VBAT_DEBOUNCE_MS: u32 = 150;
const B_OC_KILL_MA: i16 = 15_000;
const B_OV_KILL_MV: u16 = 14_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KillReason {
    Overcurrent,
    VbatSag,
    OverVolt,
    /// Bus fell more than `sag_pm` below its running baseline (a ~256 ms
    /// average of the bus; the rest voltage before a run starts).
    RelSag,
    /// Gate driver nFAULT asserted while enabled.
    DriverFault,
    /// Fast current EWMA above `oc_surge_ma` (split guard).
    Surge,
}

/// Fixed per-board guard limits — bypass the PSU/battery auto-profile.
/// Used by boards whose bench supply does not fit the L431 2S/3S
/// classification (binz: ~12 V PSU with a 700 mA current clamp).
#[derive(Clone, Copy)]
pub struct GuardLimits {
    pub vbat_floor_mv: u16,
    pub vbat_debounce_ms: u32,
    pub oc_kill_ma: i16,
    pub oc_debounce_ms: u32,
    /// Split current guard (firmware50's shape): when non-zero, the OC check
    /// runs on per-ms samples fed via `feed_current_ma` — a slow EWMA
    /// (~256 ms) against `oc_kill_ma` for a sustained overload and a fast
    /// EWMA (~16 ms) against this surge limit — instead of one short window
    /// on the 50 ms moving average, which could not tell an acceleration
    /// surge from a sustained average (binz 57.5 %: 2.24 A trip 0.18 s into
    /// a 1.85 A hold). 0 = the single check.
    pub oc_surge_ma: i16,
    pub ov_kill_mv: u16,
    pub ov_debounce_ms: u32,
    /// Relative sag kill, in permille: running bus below (1000 - sag_pm)/1000 of the rest
    /// peak. 0 = off.
    pub sag_pm: u16,
    pub sag_debounce_ms: u32,
}

/// binz limits on the bench PSU (NUCLEO-G071RB + DRV8304H, 5 A clamp):
/// 4.5 A sustained (slow EWMA) and 4.8 A surge (fast EWMA) current kills
/// below the clamp, so the firmware trips before the supply folds; >10 %
/// bus sag vs the running bus reference; 9 V absolute floor (firmware50's
/// floor); 14 V over-voltage.
pub const BINZ_BRINGUP: GuardLimits = GuardLimits {
    vbat_floor_mv: 9_000,
    vbat_debounce_ms: 5,
    oc_kill_ma: 4500, // slow EWMA (sustained), PSU clamp 5 A
    oc_debounce_ms: 1,
    oc_surge_ma: 4800, // fast EWMA, under the 5 A clamp
    ov_kill_mv: 14_000,
    ov_debounce_ms: 60,
    sag_pm: 100,
    sag_debounce_ms: 5,
};

/// binz on the 3S battery (operator, 2026-10-06): firmware50's ENV-98
/// battery spec — 8 A metered sustained allowance (slow EWMA), surge
/// ceiling 1.5x that (fast EWMA, firmware50 ENV-97), 9 V floor. Sag,
/// over-voltage and fault guards as `BINZ_BRINGUP`.
pub const BINZ_BATTERY: GuardLimits = GuardLimits {
    oc_kill_ma: 8000,
    // Operator, 2026-10-07: 12 -> 13 -> 15 A to test the 10 -> 100 % slam (killed
    // at 12 A at +0.25 s with the rotor locked). Sensing range: 7 mOhm x
    // gain 10 = 70 mV/A around VREF/2, clipping near 23 A per phase.
    oc_surge_ma: 15_000,
    // Operator, 2026-10-07: 10 % -> 15 % -> 17.5 % to test full slams on the
    // battery (10 -> 90 % passes at 15 % with a 13.2 % dip; 10 -> 100 %
    // dipped 16 % before the 12 A surge guard killed it).
    sag_pm: 175,
    ..BINZ_BRINGUP
};

/// DRV8304 wake time after ENABLE rises (datasheet tWAKE <= 1 ms); nFAULT
/// is ignored this long after the gate driver is enabled.
const FAULT_WAKE_MS: u32 = 2;
const FAULT_DEBOUNCE_MS: u32 = 1;

pub struct BenchGuard {
    cyc_per_ms: u32,
    vbat_low_since: Option<u32>,
    oc_since: Option<u32>,
    ov_since: Option<u32>,
    latched: Option<KillReason>,
    /// None until classified; then true = battery profile, false = PSU.
    battery: Option<bool>,
    /// Peak vbat observed while NOT running (rest = true open-circuit
    /// source voltage, before any spin-up sag). Classification reads
    /// this, not a single live sample — a 3S pack rests >11 V for the
    /// whole pre-arm period, the PSU rail ~8.2 V, cleanly separable
    /// with no warm-up/sag race. A single-sample classify at the first
    /// running edge misfired when spin-up inrush dipped vbat <10 V →
    /// PSU profile → OVOLT trip at 10.8 V once pack voltage recovered
    /// (measured: killed=1 on a healthy 12.2 V pack at 70 %).
    rest_peak_mv: u16,
    /// Fixed limits (None = L431 auto-profile).
    limits: Option<GuardLimits>,
    sag_since: Option<u32>,
    gate_on_since: Option<u32>,
    fault_since: Option<u32>,
    /// Split-guard EWMAs of the per-ms current, mA x256 (None = unprimed).
    ew_fast: Option<i32>,
    ew_slow: i32,
    surge_since: Option<u32>,
    /// Relative-sag reference: the bus in mV x256, a ~256 ms average while
    /// running (updated once per ms, AFTER each test) and the bus itself
    /// while stopped. 0 = unprimed.
    ew_bus: i32,
    ew_bus_at: u32,
}

impl BenchGuard {
    pub const fn new(cpu_mhz: u32) -> Self {
        Self {
            cyc_per_ms: cpu_mhz * 1000,
            vbat_low_since: None,
            oc_since: None,
            ov_since: None,
            latched: None,
            battery: None,
            rest_peak_mv: 0,
            limits: None,
            sag_since: None,
            gate_on_since: None,
            fault_since: None,
            ew_fast: None,
            ew_slow: 0,
            surge_since: None,
            ew_bus: 0,
            ew_bus_at: 0,
        }
    }

    /// Guard with fixed limits (no source classification).
    pub const fn with_limits(cpu_mhz: u32, limits: GuardLimits) -> Self {
        let mut g = Self::new(cpu_mhz);
        g.limits = Some(limits);
        g
    }

    /// Feed one per-ms current sample (mA) to the split guard's EWMAs.
    pub fn feed_current_ma(&mut self, ma: i32) {
        let x = ma.clamp(-30_000, 30_000) << 8;
        match self.ew_fast {
            None => {
                self.ew_fast = Some(x);
                self.ew_slow = x;
            }
            Some(f) => {
                self.ew_fast = Some(f + ((x - f) >> 4)); // tau ~16 ms
                self.ew_slow += (x - self.ew_slow) >> 8; // tau ~256 ms
            }
        }
    }

    /// Split-guard averages (fast, slow) in mA, for the bench report.
    pub fn current_ewmas(&self) -> (i32, i32) {
        (self.ew_fast.unwrap_or(0) >> 8, self.ew_slow >> 8)
    }

    /// Pre-run rest peak (mV), for the reports.
    pub fn rest_peak_mv(&self) -> u16 {
        self.rest_peak_mv
    }

    /// The relative-sag reference (mV) the bus is judged against.
    pub fn sag_ref_mv(&self) -> i32 {
        self.ew_bus >> 8
    }

    /// Gate-driver fault check. `gate_on` = driver ENABLE high, `fault` =
    /// nFAULT asserted. Ignored for `FAULT_WAKE_MS` after enable.
    pub fn fault_tick(&mut self, now_cyc: u32, gate_on: bool, fault: bool) -> Option<KillReason> {
        if self.latched.is_some() {
            return None;
        }
        if !gate_on {
            self.gate_on_since = None;
            self.fault_since = None;
            return None;
        }
        let since = *self.gate_on_since.get_or_insert(now_cyc);
        let awake = now_cyc.wrapping_sub(since) >= FAULT_WAKE_MS * self.cyc_per_ms;
        let r = Self::debounce(
            &mut self.fault_since,
            awake && fault,
            now_cyc,
            FAULT_DEBOUNCE_MS * self.cyc_per_ms,
            KillReason::DriverFault,
        );
        if r.is_some() {
            self.latched = r;
        }
        r
    }

    fn tick_fixed(
        &mut self,
        l: GuardLimits,
        now_cyc: u32,
        running: bool,
        vbat_mv: u16,
        current_ma: i16,
    ) -> Option<KillReason> {
        // Rest peak tracks only while stopped, so a run is judged against
        // the supply's own unloaded voltage just before it.
        if !running && vbat_mv > self.rest_peak_mv {
            self.rest_peak_mv = vbat_mv;
        }
        // Relative-sag reference (operator spec, binz 10 % cap: anchor to the
        // SYNCED operating point, not the no-load voltage; firmware50 E146
        // did the same with a ~207 ms average). Its purpose is the desync
        // current surge, which collapses the bus within milliseconds; a load
        // that draws the bus down slowly takes the reference with it, and the
        // slow direction is the absolute floor's job. On the battery the
        // pack's steady IR drop alone reached 10 % of rest at 6.9 A locked
        // (binz 97.5 %), which the rest-anchored check killed.
        let x = (vbat_mv as i32) << 8;
        if !running || self.ew_bus == 0 {
            self.ew_bus = x;
            self.ew_bus_at = now_cyc;
        }
        let bus_ref = (self.ew_bus >> 8) as u32;
        let checks = [
            (
                running && vbat_mv > 0 && vbat_mv < l.vbat_floor_mv,
                l.vbat_debounce_ms,
                KillReason::VbatSag,
            ),
            (
                running
                    && l.sag_pm > 0
                    && bus_ref > 0
                    && (vbat_mv as u32) * 1000 < bus_ref * (1000 - l.sag_pm as u32),
                l.sag_debounce_ms,
                KillReason::RelSag,
            ),
            (
                vbat_mv > l.ov_kill_mv,
                l.ov_debounce_ms,
                KillReason::OverVolt,
            ),
            (
                if l.oc_surge_ma > 0 {
                    (self.ew_slow >> 8) > l.oc_kill_ma as i32
                } else {
                    current_ma > l.oc_kill_ma
                },
                l.oc_debounce_ms,
                KillReason::Overcurrent,
            ),
            (
                l.oc_surge_ma > 0 && (self.ew_fast.unwrap_or(0) >> 8) > l.oc_surge_ma as i32,
                l.oc_debounce_ms,
                KillReason::Surge,
            ),
        ];
        // Update AFTER the test, once per ms (rate-independent tau ~256 ms),
        // so a collapsing sample cannot drag the reference down onto itself.
        if running && now_cyc.wrapping_sub(self.ew_bus_at) >= self.cyc_per_ms {
            self.ew_bus_at = now_cyc;
            self.ew_bus += (x - self.ew_bus) >> 8;
        }
        for (active, ms, reason) in checks {
            let slot = match reason {
                KillReason::VbatSag => &mut self.vbat_low_since,
                KillReason::RelSag => &mut self.sag_since,
                KillReason::OverVolt => &mut self.ov_since,
                KillReason::Surge => &mut self.surge_since,
                _ => &mut self.oc_since,
            };
            if let Some(r) = Self::debounce(slot, active, now_cyc, ms * self.cyc_per_ms, reason) {
                self.latched = Some(r);
                return Some(r);
            }
        }
        None
    }

    pub fn latched(&self) -> Option<KillReason> {
        self.latched
    }

    /// True once the source has been classified as a battery.
    pub fn battery_profile(&self) -> bool {
        self.battery == Some(true)
    }

    /// Evaluate one main-loop pass. `now_cyc` is a wrapping cycle counter
    /// (DWT.CYCCNT). Returns `Some(reason)` exactly once, on the pass that
    /// trips the kill; the latch is queried separately via `latched()`.
    ///
    /// `vbat_mv == 0` means the 1 kHz measurement block hasn't produced a
    /// reading yet (boot) — the vbat guard stays quiet rather than
    /// false-tripping on an unpopulated measurement.
    pub fn tick(
        &mut self,
        now_cyc: u32,
        running: bool,
        vbat_mv: u16,
        current_ma: i16,
    ) -> Option<KillReason> {
        if self.latched.is_some() {
            return None;
        }
        if let Some(l) = self.limits {
            return self.tick_fixed(l, now_cyc, running, vbat_mv, current_ma);
        }

        // Source classification from the REST peak (see rest_peak_mv):
        // track the highest vbat seen while not running, then lock the
        // profile at the first running edge using that settled rest
        // voltage — race-free against both the filter warm-up (starts
        // at 0) and spin-up sag (dips low under inrush). Threshold at
        // 10.5 V: a 3S pack rests >11 V, the PSU rail ~8.2 V.
        if self.battery.is_none() {
            if !running && vbat_mv > self.rest_peak_mv {
                self.rest_peak_mv = vbat_mv;
            }
            if running && self.rest_peak_mv > 0 {
                self.battery = Some(self.rest_peak_mv > 10_500);
            }
        }
        let (floor, floor_db_ms, oc_ma, ov_mv) = match self.battery {
            Some(true) => (
                B_VBAT_FLOOR_MV,
                B_VBAT_DEBOUNCE_MS,
                B_OC_KILL_MA,
                B_OV_KILL_MV,
            ),
            Some(false) => (VBAT_FLOOR_MV, VBAT_DEBOUNCE_MS, OC_KILL_MA, OV_KILL_MV),
            None => (VBAT_FLOOR_MV, VBAT_DEBOUNCE_MS, OC_KILL_MA, B_OV_KILL_MV),
        };

        let vbat_low = running && vbat_mv > 0 && vbat_mv < floor;
        if let Some(reason) = Self::debounce(
            &mut self.vbat_low_since,
            vbat_low,
            now_cyc,
            floor_db_ms * self.cyc_per_ms,
            KillReason::VbatSag,
        ) {
            self.latched = Some(reason);
            return Some(reason);
        }

        let ov = vbat_mv > ov_mv;
        if let Some(reason) = Self::debounce(
            &mut self.ov_since,
            ov,
            now_cyc,
            OV_DEBOUNCE_MS * self.cyc_per_ms,
            KillReason::OverVolt,
        ) {
            self.latched = Some(reason);
            return Some(reason);
        }

        let oc = current_ma > oc_ma;
        if let Some(reason) = Self::debounce(
            &mut self.oc_since,
            oc,
            now_cyc,
            OC_DEBOUNCE_MS * self.cyc_per_ms,
            KillReason::Overcurrent,
        ) {
            self.latched = Some(reason);
            return Some(reason);
        }

        None
    }

    fn debounce(
        since: &mut Option<u32>,
        active: bool,
        now_cyc: u32,
        threshold_cyc: u32,
        reason: KillReason,
    ) -> Option<KillReason> {
        if !active {
            *since = None;
            return None;
        }
        match *since {
            None => {
                *since = Some(now_cyc);
                None
            }
            Some(start) => {
                if now_cyc.wrapping_sub(start) >= threshold_cyc {
                    Some(reason)
                } else {
                    None
                }
            }
        }
    }
}
