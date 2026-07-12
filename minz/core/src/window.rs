//! MAGPIE/OWL float-window close — the snapshot-package-reset that
//! runs at every commutation. Extracted from the firmware so the
//! re-acquisition trigger anatomy, the boundary-prediction error,
//! the telemetry decimation policy, and (the sneaky one) the full
//! accumulator reset list are all host-tested.
//!
//! Coupling seam: the firmware's window/estimator/CL statics come in
//! as a [`WindowState`] of atomic references (same `portable-atomic`
//! types on both sides), timestamps come in as arguments, and the
//! two side effects go OUT as data in [`CloseOutcome`] — the caller
//! enqueues the record and writes the black-box events. No critical-
//! section abstraction is needed: this function never opens one; the
//! caller runs the whole thing inside its existing commutation CS
//! (so the higher/equal-priority COMP ISR can't smear an edge across
//! the old/new window during snapshot-and-reset).

use portable_atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, Ordering};

use crate::wire::WindowRec;

/// References to the firmware statics this close touches. Firmware
/// builds one `static` of these pointing at its atomics; host tests
/// build one over locals.
pub struct WindowState<'a> {
    // Per-window accumulators (reset on close).
    pub sector_start_us: &'a AtomicU32,
    pub qzc_us: &'a AtomicU32,
    pub first_zc_us: &'a AtomicU32,
    pub raw: &'a AtomicU32,
    pub valid: &'a AtomicU32,
    pub i_sum: &'a AtomicU32,
    pub i_n: &'a AtomicU32,
    pub i_min: &'a AtomicU16,
    pub i_max: &'a AtomicU16,
    pub cand_zc_us: &'a AtomicU32,
    pub window_gen: &'a AtomicU8,
    // Estimator / CL state.
    pub interval_us: &'a AtomicU32,
    pub last_qzc_us: &'a AtomicU32,
    pub windows_since_qzc: &'a AtomicU8,
    pub cl_active: &'a AtomicBool,
    pub cl_noz_run: &'a AtomicU8,
    pub cl_reacq: &'a AtomicBool,
    // Telemetry.
    pub stream_on: &'a AtomicBool,
    pub wrec_decim: &'a AtomicU32,
    pub wrec_seq: &'a AtomicU8,
    pub vbat_min: &'a AtomicU16,
    pub vbat_live: &'a AtomicU16,
    // Watchdog reference.
    pub last_comm_10us: &'a AtomicU32,
}

// Black-box event codes emitted here — authority lives in
// [`crate::blackbox`].
pub use crate::blackbox::{EV_NOZ, EV_RAQ};

/// What the caller must do after the close: enqueue `rec` (if any)
/// and record the black-box events (sector = the closed window's).
#[derive(Default)]
pub struct CloseOutcome {
    pub rec: Option<WindowRec>,
    pub bb: [Option<(u8, u16)>; 2],
}

/// Close the float window that just ended (`prev_sector`), package
/// it, and reset the accumulators for the new one. Must run inside
/// the caller's commutation critical section.
///
/// Interval smoothing lives in the COMP ISR (at the qZC itself);
/// here we only compute the boundary prediction error, drive the
/// re-acquisition trigger, and count windows for the span divider.
pub fn close_float_window(
    ws: &WindowState<'_>,
    prev_sector: u8,
    now_10us: u32,
    now_us: u32,
) -> CloseOutcome {
    let mut out = CloseOutcome::default();
    let start_us = ws.sector_start_us.load(Ordering::Relaxed);

    // Boundary prediction error: predicted = qZC + interval/2.
    // i16::MIN is the "no prediction" sentinel on the wire.
    let qzc = ws.qzc_us.load(Ordering::Relaxed);
    let mut pred_err: i16 = i16::MIN;
    if qzc != u32::MAX {
        let interval = ws.interval_us.load(Ordering::Relaxed);
        if interval != 0 {
            let err = now_us.wrapping_sub(qzc) as i64 - (interval / 2) as i64;
            pred_err = err.clamp(i16::MIN as i64 + 1, i16::MAX as i64) as i16;
        }
    }

    if qzc == u32::MAX && ws.cl_active.load(Ordering::Relaxed) {
        out.bb[0] = Some((EV_NOZ, ws.raw.load(Ordering::Relaxed).min(0xFFFF) as u16));
        // Re-acquisition trigger: only A/B windows count (phase-C
        // windows — sectors 0/3 — are dead-reckoned and legitimately
        // ZC-less).
        if prev_sector != 0 && prev_sector != 3 {
            let run = ws.cl_noz_run.load(Ordering::Relaxed).saturating_add(1);
            ws.cl_noz_run.store(run, Ordering::Relaxed);
            if run >= 2 && !ws.cl_reacq.load(Ordering::Relaxed) {
                ws.cl_reacq.store(true, Ordering::Relaxed);
                // Break the qZC chain: recovery must be measured from
                // TWO fresh strict-confirmed ZCs, not from a stale
                // pre-spiral timestamp (a stale `last` hands the
                // re-seed an aliased delta — seen re-seeding 162 µs
                // and tripping the runaway floor).
                ws.last_qzc_us.store(u32::MAX, Ordering::Relaxed);
                out.bb[1] = Some((
                    EV_RAQ,
                    ws.interval_us.load(Ordering::Relaxed).min(0xFFFF) as u16,
                ));
            }
        }
    }
    // Windowless misses no longer hard-break the estimator chain —
    // dead-reckoned C windows legitimately close without a qZC; the
    // span counter (divide-by-spans, >3 = broken) handles both cases.
    let w = ws.windows_since_qzc.load(Ordering::Relaxed);
    ws.windows_since_qzc
        .store(w.saturating_add(1), Ordering::Relaxed);

    // Telemetry decimation: at 15 k windows/s the records exceed the
    // 2 Mbaud link. Above ~925 Hz electrical (interval < 180 µs)
    // stream every 5th window — 5 is coprime with 6 so the sample
    // keeps rotating through all sectors; the host sees the seq gaps
    // and per-sector stats stay unbiased.
    let decim_n = ws.wrec_decim.fetch_add(1, Ordering::Relaxed);
    let iv_now = ws.interval_us.load(Ordering::Relaxed);
    let stream_this = iv_now == 0 || iv_now >= 180 || decim_n.is_multiple_of(5);
    if ws.stream_on.load(Ordering::Relaxed) && start_us != 0 && stream_this {
        let first_zc = ws.first_zc_us.load(Ordering::Relaxed);
        let zc_off_us = if first_zc == u32::MAX {
            0xFFFF
        } else {
            first_zc.wrapping_sub(start_us).min(0xFFFE) as u16
        };
        let qzc_off_us = if qzc == u32::MAX {
            0xFFFF
        } else {
            qzc.wrapping_sub(start_us).min(0xFFFE) as u16
        };
        let i_n = ws.i_n.load(Ordering::Relaxed);
        out.rec = Some(WindowRec {
            start_10us: start_us / 10,
            len_10us: (now_us.wrapping_sub(start_us) / 10).min(0xFFFF) as u16,
            zc_off_us,
            raw: ws.raw.load(Ordering::Relaxed).min(0xFFFF) as u16,
            valid: ws.valid.load(Ordering::Relaxed).min(0xFFFF) as u16,
            i_min: if i_n == 0 {
                0
            } else {
                ws.i_min.load(Ordering::Relaxed)
            },
            i_max: ws.i_max.load(Ordering::Relaxed),
            i_avg: ws
                .i_sum
                .load(Ordering::Relaxed)
                .checked_div(i_n)
                .unwrap_or(0) as u16,
            qzc_off_us,
            pred_err_us: pred_err,
            vbat_raw: {
                // Min-fold across decimation gaps: worst vbat since
                // the last STREAMED window; live sample as fallback.
                let m = ws.vbat_min.swap(u16::MAX, Ordering::Relaxed);
                if m == u16::MAX {
                    ws.vbat_live.load(Ordering::Relaxed)
                } else {
                    m
                }
            },
            sector: prev_sector,
            seq: ws.wrec_seq.fetch_add(1, Ordering::Relaxed),
        });
    }

    // Reset every accumulator for the new window. A missing line
    // here is silent telemetry corruption — the host test pins the
    // complete list.
    ws.raw.store(0, Ordering::Relaxed);
    ws.valid.store(0, Ordering::Relaxed);
    ws.first_zc_us.store(u32::MAX, Ordering::Relaxed);
    ws.qzc_us.store(u32::MAX, Ordering::Relaxed);
    ws.i_sum.store(0, Ordering::Relaxed);
    ws.i_n.store(0, Ordering::Relaxed);
    ws.i_min.store(0x0FFF, Ordering::Relaxed);
    ws.i_max.store(0, Ordering::Relaxed);
    ws.cand_zc_us.store(u32::MAX, Ordering::Relaxed);
    ws.window_gen.store(
        ws.window_gen.load(Ordering::Relaxed).wrapping_add(1),
        Ordering::Relaxed,
    );
    ws.sector_start_us.store(now_us, Ordering::Relaxed);
    ws.last_comm_10us.store(now_10us, Ordering::Relaxed);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owned backing store for a WindowState, pre-loaded with a
    /// healthy mid-window snapshot.
    struct Rig {
        sector_start_us: AtomicU32,
        qzc_us: AtomicU32,
        first_zc_us: AtomicU32,
        raw: AtomicU32,
        valid: AtomicU32,
        i_sum: AtomicU32,
        i_n: AtomicU32,
        i_min: AtomicU16,
        i_max: AtomicU16,
        cand_zc_us: AtomicU32,
        window_gen: AtomicU8,
        interval_us: AtomicU32,
        last_qzc_us: AtomicU32,
        windows_since_qzc: AtomicU8,
        cl_active: AtomicBool,
        cl_noz_run: AtomicU8,
        cl_reacq: AtomicBool,
        stream_on: AtomicBool,
        wrec_decim: AtomicU32,
        wrec_seq: AtomicU8,
        vbat_min: AtomicU16,
        vbat_live: AtomicU16,
        last_comm_10us: AtomicU32,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                sector_start_us: AtomicU32::new(10_000),
                qzc_us: AtomicU32::new(10_300), // qZC 300 µs into window
                first_zc_us: AtomicU32::new(10_250),
                raw: AtomicU32::new(40),
                valid: AtomicU32::new(6),
                i_sum: AtomicU32::new(3_000),
                i_n: AtomicU32::new(30),
                i_min: AtomicU16::new(80),
                i_max: AtomicU16::new(140),
                cand_zc_us: AtomicU32::new(10_310),
                window_gen: AtomicU8::new(7),
                interval_us: AtomicU32::new(600),
                last_qzc_us: AtomicU32::new(10_300),
                windows_since_qzc: AtomicU8::new(0),
                cl_active: AtomicBool::new(false),
                cl_noz_run: AtomicU8::new(0),
                cl_reacq: AtomicBool::new(false),
                stream_on: AtomicBool::new(true),
                wrec_decim: AtomicU32::new(0),
                wrec_seq: AtomicU8::new(0),
                vbat_min: AtomicU16::new(u16::MAX),
                vbat_live: AtomicU16::new(1_083),
                last_comm_10us: AtomicU32::new(1_000),
            }
        }

        fn state(&self) -> WindowState<'_> {
            WindowState {
                sector_start_us: &self.sector_start_us,
                qzc_us: &self.qzc_us,
                first_zc_us: &self.first_zc_us,
                raw: &self.raw,
                valid: &self.valid,
                i_sum: &self.i_sum,
                i_n: &self.i_n,
                i_min: &self.i_min,
                i_max: &self.i_max,
                cand_zc_us: &self.cand_zc_us,
                window_gen: &self.window_gen,
                interval_us: &self.interval_us,
                last_qzc_us: &self.last_qzc_us,
                windows_since_qzc: &self.windows_since_qzc,
                cl_active: &self.cl_active,
                cl_noz_run: &self.cl_noz_run,
                cl_reacq: &self.cl_reacq,
                stream_on: &self.stream_on,
                wrec_decim: &self.wrec_decim,
                wrec_seq: &self.wrec_seq,
                vbat_min: &self.vbat_min,
                vbat_live: &self.vbat_live,
                last_comm_10us: &self.last_comm_10us,
            }
        }

        fn close(&self, sector: u8, now_10us: u32, now_us: u32) -> CloseOutcome {
            close_float_window(&self.state(), sector, now_10us, now_us)
        }
    }

    #[test]
    fn healthy_window_packages_record_and_pred_err() {
        let r = Rig::new();
        // Close at 10 600 µs: window len 600, qZC at +300, predicted
        // boundary = qZC + 600/2 → err = 600 - 300 - 300 = 0.
        let out = r.close(1, 2_000, 10_600);
        let rec = out.rec.expect("record");
        assert_eq!(rec.sector, 1);
        assert_eq!(rec.start_10us, 1_000);
        assert_eq!(rec.len_10us, 60);
        assert_eq!(rec.zc_off_us, 250);
        assert_eq!(rec.qzc_off_us, 300);
        assert_eq!(rec.pred_err_us, 0);
        assert_eq!(rec.raw, 40);
        assert_eq!(rec.valid, 6);
        assert_eq!(rec.i_avg, 100);
        assert_eq!(rec.i_min, 80);
        assert_eq!(rec.vbat_raw, 1_083, "vbat falls back to live sample");
        assert_eq!(rec.seq, 0);
        assert!(out.bb.iter().all(Option::is_none), "no bb events");
    }

    #[test]
    fn accumulators_fully_reset_for_next_window() {
        let r = Rig::new();
        r.vbat_min.store(900, Ordering::Relaxed);
        r.close(1, 2_000, 10_600);
        assert_eq!(r.raw.load(Ordering::Relaxed), 0);
        assert_eq!(r.valid.load(Ordering::Relaxed), 0);
        assert_eq!(r.first_zc_us.load(Ordering::Relaxed), u32::MAX);
        assert_eq!(r.qzc_us.load(Ordering::Relaxed), u32::MAX);
        assert_eq!(r.i_sum.load(Ordering::Relaxed), 0);
        assert_eq!(r.i_n.load(Ordering::Relaxed), 0);
        assert_eq!(r.i_min.load(Ordering::Relaxed), 0x0FFF);
        assert_eq!(r.i_max.load(Ordering::Relaxed), 0);
        assert_eq!(r.cand_zc_us.load(Ordering::Relaxed), u32::MAX);
        assert_eq!(r.window_gen.load(Ordering::Relaxed), 8, "gen bumped");
        assert_eq!(r.sector_start_us.load(Ordering::Relaxed), 10_600);
        assert_eq!(r.last_comm_10us.load(Ordering::Relaxed), 2_000);
        assert_eq!(
            r.vbat_min.load(Ordering::Relaxed),
            u16::MAX,
            "min-fold consumed"
        );
        assert_eq!(r.windows_since_qzc.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn no_qzc_no_interval_pred_err_is_sentinel() {
        let r = Rig::new();
        r.qzc_us.store(u32::MAX, Ordering::Relaxed);
        let out = r.close(1, 2_000, 10_600);
        assert_eq!(out.rec.unwrap().pred_err_us, i16::MIN);
        let r = Rig::new();
        r.interval_us.store(0, Ordering::Relaxed);
        let out = r.close(1, 2_000, 10_600);
        assert_eq!(out.rec.unwrap().pred_err_us, i16::MIN);
    }

    #[test]
    fn reacq_trigger_two_zcless_ab_windows() {
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        // First ZC-less A/B window: NOZ event, run=1, no reacq yet.
        r.qzc_us.store(u32::MAX, Ordering::Relaxed);
        let out = r.close(1, 2_000, 10_600);
        assert_eq!(out.bb[0], Some((EV_NOZ, 40)));
        assert_eq!(out.bb[1], None);
        assert!(!r.cl_reacq.load(Ordering::Relaxed));
        // Second: reacq engages, chain broken, RAQ event with the
        // interval snapshot.
        r.qzc_us.store(u32::MAX, Ordering::Relaxed);
        let out = r.close(2, 2_060, 11_200);
        assert_eq!(out.bb[1], Some((EV_RAQ, 600)));
        assert!(r.cl_reacq.load(Ordering::Relaxed));
        assert_eq!(
            r.last_qzc_us.load(Ordering::Relaxed),
            u32::MAX,
            "stale pre-spiral timestamp must not survive into the re-seed"
        );
    }

    #[test]
    fn dead_reckoned_c_windows_exempt_from_reacq() {
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        for (i, sec) in [0u8, 3, 0, 3].iter().enumerate() {
            r.qzc_us.store(u32::MAX, Ordering::Relaxed);
            let out = r.close(*sec, 2_000 + i as u32, 10_600 + 600 * i as u32);
            assert_eq!(out.bb[0].map(|e| e.0), Some(EV_NOZ), "NOZ still logged");
            assert_eq!(out.bb[1], None);
        }
        assert_eq!(r.cl_noz_run.load(Ordering::Relaxed), 0);
        assert!(!r.cl_reacq.load(Ordering::Relaxed));
    }

    #[test]
    fn open_loop_zcless_windows_do_not_reacq() {
        let r = Rig::new(); // cl_active = false
        for i in 0..4u32 {
            r.qzc_us.store(u32::MAX, Ordering::Relaxed);
            let out = r.close(1, 2_000 + i, 10_600 + 600 * i);
            assert!(out.bb.iter().all(Option::is_none));
        }
        assert!(!r.cl_reacq.load(Ordering::Relaxed));
    }

    #[test]
    fn decimation_full_rate_below_925hz_fifth_above() {
        let r = Rig::new();
        // Slow (600 µs ≥ 180): every window streams.
        for i in 0..7u32 {
            let out = r.close(1, 2_000 + i, 10_600 + 600 * i);
            assert!(out.rec.is_some(), "window {i} dropped at low speed");
        }
        // Fast (150 µs): only decim counter multiples of 5.
        r.interval_us.store(150, Ordering::Relaxed);
        let mut streamed = 0;
        for i in 0..20u32 {
            if r.close(1, 3_000 + i, 20_000 + 150 * i).rec.is_some() {
                streamed += 1;
            }
        }
        assert_eq!(streamed, 4, "every 5th of 20");
    }

    #[test]
    fn stream_off_or_unseeded_start_still_resets() {
        let r = Rig::new();
        r.stream_on.store(false, Ordering::Relaxed);
        let out = r.close(1, 2_000, 10_600);
        assert!(out.rec.is_none());
        assert_eq!(r.sector_start_us.load(Ordering::Relaxed), 10_600);
        let r = Rig::new();
        r.sector_start_us.store(0, Ordering::Relaxed);
        let out = r.close(1, 2_000, 10_600);
        assert!(out.rec.is_none(), "boot window (start=0) never streams");
    }

    #[test]
    fn zero_current_samples_yield_zero_not_stale_or_panic() {
        let r = Rig::new();
        r.i_n.store(0, Ordering::Relaxed);
        let rec = r.close(1, 2_000, 10_600).rec.unwrap();
        assert_eq!(rec.i_avg, 0);
        assert_eq!(rec.i_min, 0, "sentinel 0x0FFF must not leak");
    }

    #[test]
    fn vbat_min_fold_prefers_worst_since_last_stream() {
        let r = Rig::new();
        r.vbat_min.store(620, Ordering::Relaxed); // deep sag seen
        let rec = r.close(1, 2_000, 10_600).rec.unwrap();
        assert_eq!(
            rec.vbat_raw, 620,
            "sag must reach the wire, not the live value"
        );
    }

    #[test]
    fn windows_since_qzc_saturates() {
        let r = Rig::new();
        r.windows_since_qzc.store(255, Ordering::Relaxed);
        r.close(1, 2_000, 10_600);
        assert_eq!(r.windows_since_qzc.load(Ordering::Relaxed), 255);
    }
}
