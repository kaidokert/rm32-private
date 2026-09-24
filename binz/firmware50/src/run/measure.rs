//! The run's bridge-off measurements: the preflight line, the pre-drive
//! baseline and zero, and the coast witness. Each is the binary's code as it
//! stood at E118, generic over [`Hal`].

use crate::fixed::div_100;
use crate::protection::{ADC_RAIL, BLOCK_SCANS, BusReference, ZERO_BLOCKS, zero_from_blocks};
use crate::report::{CoastStats, Sink};

use super::hal::{Hal, Preflight};
use super::policy::{
    BUS_DIVIDER_X100, BUS_FLOOR_MV, COAST_COMP_HYST, COAST_DEBOUNCE_US, COAST_HYST_CODES, COAST_WINDOW_MS,
    CSA_BIAS_MAX, CSA_BIAS_MIN,
};
use crate::commutation::Step;

/// Write the `PREFLIGHT` line for a readback.
pub fn say_preflight(p: &Preflight, out: &mut impl Sink) {
    out.say("PREFLIGHT ");
    out.kv("moe", u32::from(p.moe));
    out.kv("ccr1", p.ccr[0]);
    out.kv("ccr2", p.ccr[1]);
    out.kv("ccr3", p.ccr[2]);
    out.kv("gates_low", u32::from(p.gates_low));
    out.kv("en", u32::from(p.enable));
    out.kv("nfault", u32::from(p.nfault_high));
    out.say(if p.passed() {
        "verdict=PASS\r\n"
    } else {
        "verdict=FAIL\r\n"
    });
}

/// The same-wake reference the protections measure against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Baseline {
    pub zero_block: u32,
    pub bus_ref: BusReference,
    /// Raw bus code equivalent to [`BUS_FLOOR_MV`], computed once per run.
    pub bus_floor_code: u16,
}

/// Capture the pre-drive baseline, bridge OFF: a baseline taken while driving
/// would bake the drive current into the zero and the limit would never trip.
///
/// Refuses (and says why on `out`) unless each current-sense amplifier's
/// undriven average is plausible for a *biased* amplifier: captured with
/// ENABLE low they read near 0, and the over-current protection would be
/// silently disarmed.
pub fn capture_baseline(hal: &mut (impl Hal + Sink)) -> Option<Baseline> {
    let mut sum = 0u32;
    let mut sums = [0u32; 5];
    let mut n = 0u32;
    let mut guard = 0u32;
    while n < BLOCK_SCANS && guard < 2_000_000 {
        guard += 1;
        hal.now();
        if !hal.adc_due() {
            continue;
        }
        let s = hal.scan()?;
        sum = sum
            .saturating_add(u32::from(s.phase_a))
            .saturating_add(u32::from(s.phase_b))
            .saturating_add(u32::from(s.phase_c));
        sums[0] += u32::from(s.phase_a);
        sums[1] += u32::from(s.phase_b);
        sums[2] += u32::from(s.phase_c);
        sums[3] += u32::from(s.bus);
        sums[4] += u32::from(s.vref);
        n += 1;
    }
    if n != BLOCK_SCANS {
        return None;
    }
    let avg = [div_100(sums[0]), div_100(sums[1]), div_100(sums[2])];
    if avg.iter().any(|a| !(CSA_BIAS_MIN..=CSA_BIAS_MAX).contains(a)) {
        hal.say("ABORT csa_bias_implausible ");
        hal.kv("a", avg[0]);
        hal.kv("b", avg[1]);
        hal.kv("c", avg[2]);
        hal.kv("min", CSA_BIAS_MIN);
        hal.kv("max", CSA_BIAS_MAX);
        hal.say("\r\n");
        return None;
    }
    // VDDA from the averaged VREFINT code and the factory calibration, then
    // the bus floor as a raw code, once per run and bridge off (the only
    // runtime-divisor divisions, `__aeabi_uidiv`, reachable from no root).
    let vdda_mv = hal.vdda_mv(div_100(sums[4]) as u16);
    if vdda_mv == 0 {
        return None;
    }
    let numer = BUS_FLOOR_MV * 100 * u32::from(ADC_RAIL);
    let floor_code = numer
        .checked_div(BUS_DIVIDER_X100 * vdda_mv)
        .unwrap_or(u32::from(ADC_RAIL));
    Some(Baseline {
        zero_block: sum,
        bus_ref: BusReference {
            bus: div_100(sums[3]) as u16,
            vref: div_100(sums[4]) as u16,
        },
        bus_floor_code: floor_code.min(u32::from(ADC_RAIL)) as u16,
    })
}

/// The current proxy's datum averaged over `ZERO_BLOCKS` bridge-off blocks
/// (E093), `first` being the block `capture_baseline` already took.
pub fn averaged_zero(hal: &mut (impl Hal + Sink), first: u32) -> Option<u32> {
    let mut sum = u64::from(first);
    let mut n = 1u32;
    while n < ZERO_BLOCKS {
        sum += u64::from(capture_baseline(hal)?.zero_block);
        n += 1;
    }
    zero_from_blocks(sum, n)
}

/// Debounced comparator transitions over the coast (E065): a level that
/// disagrees with the held one for `COAST_DEBOUNCE_US` is a transition,
/// stamped where it began.
struct Debounce {
    held: bool,
    cand_since: Option<u32>,
    last: Option<u32>,
}

impl Debounce {
    fn feed(&mut self, lvl: bool, now: u32, start: u32, st: &mut CoastStats) {
        if lvl == self.held {
            self.cand_since = None;
            return;
        }
        match self.cand_since {
            None => self.cand_since = Some(now),
            Some(t0) if now.wrapping_sub(t0) >= COAST_DEBOUNCE_US => {
                self.held = lvl;
                self.cand_since = None;
                match self.last {
                    None => st.first_trans_us = t0.wrapping_sub(start),
                    Some(prev) => {
                        let k = (st.trans_n as usize).wrapping_sub(1);
                        if k < st.trans_iv.len() {
                            st.trans_iv[k] = t0.wrapping_sub(prev);
                        }
                    }
                }
                self.last = Some(t0);
                st.trans_n = st.trans_n.saturating_add(1);
            }
            Some(_) => {}
        }
    }
}

/// ADC-BEMF polarity alternations against the signal's own midpoint, with the
/// first- and last-quarter peak-to-peak.
struct Swing {
    first: (i32, i32),
    last: (i32, i32),
    midpoint: i32,
    mid_fixed: bool,
    state: i8,
}

impl Swing {
    fn feed(&mut self, v: i32, elapsed: u32, quarter_us: u32, st: &mut CoastStats) {
        if elapsed < quarter_us {
            self.first = (self.first.0.min(v), self.first.1.max(v));
        } else if elapsed >= quarter_us * 3 {
            self.last = (self.last.0.min(v), self.last.1.max(v));
        }
        if !self.mid_fixed {
            if elapsed >= quarter_us && self.first.1 > self.first.0 {
                self.midpoint = (self.first.1 + self.first.0) / 2;
                self.mid_fixed = true;
            }
            return;
        }
        let d = v - self.midpoint;
        if d > COAST_HYST_CODES {
            if self.state == -1 {
                st.crossings += 1;
            }
            self.state = 1;
        } else if d < -COAST_HYST_CODES {
            if self.state == 1 {
                st.crossings += 1;
            }
            self.state = -1;
        }
    }
}

/// Watch the freewheeling bridge for `COAST_WINDOW_MS`: the rotation witness
/// that depends on nothing in the control path. Comparator on phase C at the
/// coast hysteresis, restored to the run's after.
pub fn coast_capture(hal: &mut impl Hal, stopped_at: u32) -> CoastStats {
    hal.float_all();
    hal.comp_select(Step::new_clamped(1));
    hal.comp_hysteresis(COAST_COMP_HYST);
    let start = hal.now();
    let quarter_us = COAST_WINDOW_MS * 1_000 / 4;
    let mut comp_state = hal.comp_level();
    let mut deb = Debounce {
        held: comp_state,
        cand_since: None,
        last: None,
    };
    let mut swing = Swing {
        first: (i32::MAX, i32::MIN),
        last: (i32::MAX, i32::MIN),
        midpoint: 0,
        mid_fixed: false,
        state: 0,
    };
    let mut st = CoastStats::empty();
    // The coast's origin against the stop's: what a back-extrapolation to the
    // bridge-off instant has to cross (E155).
    st.offset_us = start.wrapping_sub(stopped_at);
    loop {
        let now = hal.now();
        let elapsed = now.wrapping_sub(start);
        if elapsed >= COAST_WINDOW_MS * 1_000 {
            break;
        }
        hal.drain();
        // Polled every iteration: the ADC cadence would alias the edges.
        let lvl = hal.comp_level();
        st.comp_polls = st.comp_polls.saturating_add(1);
        if lvl {
            st.comp_hi = st.comp_hi.saturating_add(1);
        }
        if lvl != comp_state {
            st.comp_edges = st.comp_edges.saturating_add(1);
            comp_state = lvl;
        }
        deb.feed(lvl, now, start, &mut st);
        if !hal.adc_due() || hal.scan().is_none() {
            continue;
        }
        st.scans += 1;
        let (vsenc, star) = hal.comp_inputs();
        swing.feed(i32::from(vsenc) - i32::from(star), elapsed, quarter_us, &mut st);
    }
    st.midpoint = swing.midpoint;
    hal.comp_run_hysteresis();
    if swing.first.1 > swing.first.0 {
        st.pp_first = swing.first.1 - swing.first.0;
    }
    if swing.last.1 > swing.last.0 {
        st.pp_last = swing.last.1 - swing.last.0;
    }
    st
}
