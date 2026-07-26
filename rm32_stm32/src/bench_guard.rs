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

/// Battery-source profile (3S pack, clone-side numbers from the
/// battB_ sessions): floor 8.47 V, OC 15 A (slam accel measured ~10 A
/// average — a real operating point, 5x stall margin), OV 14.0 V
/// (pack rest tops ~12.6 V; regen charges the pack instead of pumping,
/// so OV only guards a genuinely wrong source state). Selected
/// automatically by the first vbat reading: >10 V rest = battery.
const B_VBAT_FLOOR_MV: u16 = 8_470;
const B_OC_KILL_MA: i16 = 15_000;
const B_OV_KILL_MV: u16 = 14_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KillReason {
    Overcurrent,
    VbatSag,
    OverVolt,
}

pub struct BenchGuard {
    cyc_per_ms: u32,
    vbat_low_since: Option<u32>,
    oc_since: Option<u32>,
    ov_since: Option<u32>,
    latched: Option<KillReason>,
    /// None until the first nonzero vbat sample; then true = battery
    /// profile (rest >10 V), false = bench-PSU profile.
    battery: Option<bool>,
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
        }
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

        // Source classification at the FIRST running edge — not the first
        // vbat sample: the measurement filter warms up from 0, so an early
        // sample reads low, classifies a battery bench as PSU, and the
        // filter then climbs through the PSU OV line on its way to pack
        // voltage (measured: killed=1 within 2 s of boot on a 12.17 V
        // pack). By the time the motor runs, the filter has settled for
        // over a second and regen (the thing OV guards against) is not
        // yet possible. Until classified, thresholds are cross-safe: PSU
        // floor + PSU OC (battery idle never near either) with battery
        // OV (a PSU can't pump while the motor has never run).
        if self.battery.is_none() && running && vbat_mv > 0 {
            self.battery = Some(vbat_mv > 10_000);
        }
        let (floor, oc_ma, ov_mv) = match self.battery {
            Some(true) => (B_VBAT_FLOOR_MV, B_OC_KILL_MA, B_OV_KILL_MV),
            Some(false) => (VBAT_FLOOR_MV, OC_KILL_MA, OV_KILL_MV),
            None => (VBAT_FLOOR_MV, OC_KILL_MA, B_OV_KILL_MV),
        };

        let vbat_low = running && vbat_mv > 0 && vbat_mv < floor;
        if let Some(reason) = Self::debounce(
            &mut self.vbat_low_since,
            vbat_low,
            now_cyc,
            VBAT_DEBOUNCE_MS * self.cyc_per_ms,
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
