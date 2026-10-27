//! Bench (`debuguart`): one `[run]` line per throttle session, printed
//! from main once the motor has stopped AND the input has sat at zero for
//! 0.1 s (or the signal-timeout reset ends it: `finish`), so nothing
//! prints during a powered run. Lets an FC-driven test
//! tell a closed-loop spin from a motor that only kept attempting starts:
//!
//! `[run] starts=N ms=T zc_pk=Z ecom_min=E erpm_pk=R dsy=D fast= cur= slow= old= orbit= comms=`
//!
//! - `starts`: running rising edges in the session (1 = clean; more =
//!   stop/restart cycles, the choke signature)
//! - `ms`: total time running
//! - `zc_pk`: peak zero-cross count (BEMF-detected commutations, AM32's
//!   10000 cap); thousands = closed loop
//! - `ecom_min`, `erpm_pk`: shortest electrical period (us, the value the
//!   bidir eRPM report encodes) once zc >= 100, and 60e6 / it
//! - `dsy`: desync events in the session (`desync_events`), then the
//!   desync branches (fast-rotor stay, demote on current / slow / from
//!   polling) and orbit-current trips, all session deltas
//! - `comms`: locked (closed-loop) commutations in the session
//! - `first=L/A/CI/ZC`: the desync detector's inputs at the session's
//!   first desync (last_average_interval, average_interval,
//!   commutation_interval, zero_crosses); `first=-` when none

/// Ticks (20 kHz) of stopped + zero input that end a session.
const QUIET_TICKS: u32 = 2_000;
/// Speed samples count only once the BEMF loop has this many crossings.
const ECOM_MIN_ZC: u32 = 100;

#[derive(Default)]
pub struct RunSummary {
    active: bool,
    was_running: bool,
    starts: u32,
    run_ticks: u32,
    last_tick: u32,
    quiet_since: u32,
    zc_pk: u32,
    ecom_min: u32,
    c0: [u32; 7],
    first: Option<[u32; 4]>,
    hist: [[u32; 4]; 4],
}

impl RunSummary {
    pub const fn new() -> Self {
        Self {
            active: false,
            was_running: false,
            starts: 0,
            run_ticks: 0,
            last_tick: 0,
            quiet_since: 0,
            zc_pk: 0,
            ecom_min: u32::MAX,
            c0: [0; 7],
            first: None,
            hist: [[0; 4]; 4],
        }
    }

    /// Call once per main-loop pass.
    pub fn poll(
        &mut self,
        tick: u32,
        running: bool,
        newinput: u16,
        zc: u32,
        e_com_time: i32,
        c: [u32; 7],
        dsy_snap: [u32; 4],
        dsy_hist: [[u32; 4]; 4],
    ) {
        let dt = tick.wrapping_sub(self.last_tick);
        self.last_tick = tick;
        if running && !self.was_running {
            if !self.active {
                *self = Self::new();
                self.active = true;
                self.c0 = c;
                self.last_tick = tick;
            }
            self.starts += 1;
        }
        self.was_running = running;
        if !self.active {
            return;
        }
        // Main polls far faster than desyncs repeat (~11 ms apart in a
        // choke), so the snapshot seen with the first count is the first.
        if self.first.is_none() && c[0] != self.c0[0] {
            self.first = Some(dsy_snap);
            self.hist = dsy_hist;
        }
        if running {
            self.run_ticks = self.run_ticks.wrapping_add(dt);
            self.zc_pk = self.zc_pk.max(zc);
            if zc >= ECOM_MIN_ZC && e_com_time > 0 {
                self.ecom_min = self.ecom_min.min(e_com_time as u32);
            }
        }
        if running || newinput != 0 {
            self.quiet_since = tick;
            return;
        }
        if tick.wrapping_sub(self.quiet_since) < QUIET_TICKS {
            return;
        }
        self.finish(c);
    }

    /// Print an open session now (the signal-timeout reset path: an FC
    /// reboot right after a hold resets the ESC before the quiet window).
    pub fn finish(&mut self, c: [u32; 7]) {
        if !self.active {
            return;
        }
        let (ecom, erpm) = if self.ecom_min == u32::MAX {
            (0, 0)
        } else {
            (self.ecom_min, 60_000_000 / self.ecom_min.max(1))
        };
        crate::dprintln!(
            "[run] starts={} ms={} zc_pk={} ecom_min={} erpm_pk={} dsy={} fast={} cur={} slow={} old={} orbit={} comms={} first={} hist={}",
            self.starts,
            self.run_ticks / 20,
            self.zc_pk,
            ecom,
            erpm,
            c[0].wrapping_sub(self.c0[0]),
            c[1].wrapping_sub(self.c0[1]),
            c[2].wrapping_sub(self.c0[2]),
            c[3].wrapping_sub(self.c0[3]),
            c[4].wrapping_sub(self.c0[4]),
            c[5].wrapping_sub(self.c0[5]),
            c[6].wrapping_sub(self.c0[6]),
            First(self.first),
            Hist(self.hist),
        );
        self.active = false;
    }
}

struct First(Option<[u32; 4]>);

impl core::fmt::Display for First {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Some([l, a, ci, zc]) => write!(f, "{}/{}/{}/{}", l, a, ci, zc),
            None => f.write_str("-"),
        }
    }
}

/// `zc:avg:duty:dmax` per check, oldest first; `p` marks polling mode.
struct Hist([[u32; 4]; 4]);

impl core::fmt::Display for Hist {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, [zp, avg, duty, dmax]) in self.0.iter().enumerate() {
            let sep = if i == 0 { "" } else { "," };
            let p = if zp >> 31 != 0 { "p" } else { "" };
            write!(
                f,
                "{}{}{}:{}:{}:{}",
                sep,
                zp & 0x7fff_ffff,
                p,
                avg,
                duty,
                dmax
            )?;
        }
        Ok(())
    }
}
