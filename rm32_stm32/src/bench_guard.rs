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
const OV_KILL_MV: u16 = 10_200;
const OV_DEBOUNCE_MS: u32 = 5;

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
}

impl BenchGuard {
    pub const fn new(cpu_mhz: u32) -> Self {
        Self {
            cyc_per_ms: cpu_mhz * 1000,
            vbat_low_since: None,
            oc_since: None,
            ov_since: None,
            latched: None,
        }
    }

    pub fn latched(&self) -> Option<KillReason> {
        self.latched
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

        let vbat_low = running && vbat_mv > 0 && vbat_mv < VBAT_FLOOR_MV;
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

        let ov = vbat_mv > OV_KILL_MV;
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

        let oc = current_ma > OC_KILL_MA;
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
