//! Closed-loop simulation: run the real `rinz::cl::ClLoop` controller against a
//! simple host motor model (Stage 1b, path A of CLOSED_LOOP_PLAN.md). The model's
//! BEMF responds to the loop's commutation, so this catches the gross loop bugs that
//! observe-only replay cannot: sign errors, runaway, failure to acquire/hold lock.
//! It is NOT a fidelity claim — it's a cheap logic shakedown before bounded hardware.
//!
//! Model: a rotor (electrical angle phi, speed omega deg/tick) with simple six-step
//! torque `Kt*sin(field-phi)` minus a constant load and viscous friction. The floating
//! phase's BEMF crosses zero at the sector midpoint (trapezoidal flat-top), plus a
//! post-commutation demag spike and measurement noise. A LOAD STEP partway through
//! forces a real speed change so the loop must TRACK via the ZC — a constant-speed
//! rotor is dead-reckonable, so the step is what makes "stays locked" meaningful.
//!
//! Two tests:
//!   cl_sim_locks    — with ZC feedback (blank=2): locks, tracks the step, period <10%.
//!   cl_sim_needs_zc — negative control (blank huge -> no ZC): MUST desync/stall, i.e.
//!                     the loop is genuinely closing on the ZC, not dead-reckoning.
//!
//! Run via `cargo cl-sim`. cl_sim_locks emits logs/cl_sim.txt for cl_sim_plot.py.

use rinz::cl::{ClLoop, SIX_HIGH, SIX_LOW};

const VBUS: i32 = 1650;
const NEUTRAL: f32 = 825.0;
const A_BEMF: f32 = 200.0;

fn float_phase(s: u8) -> usize {
    3 - SIX_HIGH[s as usize] - SIX_LOW[s as usize]
}
fn sind(deg: f32) -> f32 {
    deg.to_radians().sin()
}
fn envf(k: &str, d: f32) -> f32 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

struct Motor {
    phi: f32,
    total: f32,
    omega: f32,
    kt: f32,
    load: f32,
    visc: f32,
    j: f32,
    trap: f32,
    demag: f32,
    demag_tau: f32,
    noise: f32,
    rng: u32,
    fsc: u32,
}

impl Motor {
    fn new(omega0: f32) -> Self {
        Motor {
            phi: 0.0,
            total: 0.0,
            omega: omega0,
            kt: envf("SIM_KT", 0.6),
            load: envf("SIM_LOAD", 0.28),
            visc: envf("SIM_VISC", 0.05),
            j: envf("SIM_J", 200.0),
            trap: envf("SIM_TRAP", 150.0),
            demag: envf("SIM_DEMAG", 300.0),
            demag_tau: envf("SIM_DEMAG_TAU", 1.5),
            noise: envf("SIM_NOISE", 12.0),
            rng: 0x1234_5678,
            fsc: 0,
        }
    }
    fn step(&mut self, sector: u8) {
        let field = sector as f32 * 60.0 + 30.0 + 90.0;
        let torque = self.kt * sind(field - self.phi) - self.load - self.visc * self.omega;
        self.omega += torque / self.j;
        if self.omega < 0.1 {
            self.omega = 0.1;
        }
        self.phi = (self.phi + self.omega) % 360.0;
        self.total += self.omega;
        self.fsc += 1;
    }
    fn noise_sample(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        if self.noise <= 0.0 {
            return 0.0;
        }
        (((self.rng >> 8) & 0xffff) as f32 / 65535.0 - 0.5) * 2.0 * self.noise
    }
    fn bemf(&mut self, sector: u8) -> [i32; 3] {
        let f = float_phase(sector);
        let (hi, lo) = (SIX_HIGH[sector as usize], SIX_LOW[sector as usize]);
        let mut e = A_BEMF * sind(self.phi - (sector as f32 * 60.0 + 30.0));
        if self.trap > 0.0 {
            e = e.clamp(-self.trap, self.trap);
        }
        if self.demag > 0.0 {
            e += self.demag * (-(self.fsc as f32) / self.demag_tau).exp();
        }
        e += self.noise_sample();
        let mut b = [0i32; 3];
        b[hi] = VBUS + self.noise_sample() as i32;
        b[lo] = self.noise_sample() as i32;
        b[f] = (NEUTRAL + e) as i32;
        b
    }
    fn true_period(&self) -> f32 {
        60.0 / self.omega
    }
}

struct SimResult {
    omega_end: f32,
    sync: f32, // commutations / rotor_sectors (1.0 = perfectly synced)
    coast_frac: f32,
    post_perr: f32, // median |period_est - true| after the load step
}

/// Run the closed loop against the model with the given demag-blank. `trace` (if Some)
/// writes the per-tick trajectory for plotting.
fn run_sim(blank: u32, trace: Option<&str>) -> SimResult {
    let omega0 = envf("SIM_OMEGA", 5.4);
    let mut m = Motor::new(omega0);
    let mut lp = ClLoop::new(
        60.0 / omega0,
        envf("CL_KP", 0.3),
        envf("CL_KI", 0.2),
        envf("CL_GATE", 0.5),
        envf("CL_COAST", 1.0), // dead-reckon at period_est when a ZC is missed
        blank,
    );
    let (n, warm, step_tick) = (6000usize, 4500usize, 3000usize);
    let load_step = envf("SIM_LOAD_STEP", 0.15);

    let mut log =
        String::from("# tick phi omega sector period_est true_period commutate zc coasted\n");
    let (mut commutations, mut coasts) = (0u32, 0u32);
    let mut perr = Vec::new();
    for tick in 0..n {
        if tick == step_tick {
            m.load += load_step;
        }
        m.step(lp.sector());
        let bemf = m.bemf(lp.sector());
        let step = lp.on_frame(bemf);
        if step.commutate {
            commutations += 1;
            m.fsc = 0;
            if step.coasted {
                coasts += 1;
            }
        }
        if tick >= warm {
            perr.push((step.period_est - m.true_period()).abs() / m.true_period());
        }
        if trace.is_some() {
            log.push_str(&format!(
                "{} {:.2} {:.4} {} {:.2} {:.2} {} {} {}\n",
                tick,
                m.phi,
                m.omega,
                step.sector,
                step.period_est,
                m.true_period(),
                step.commutate as u8,
                step.zc_ticks.map(|z| z as i32).unwrap_or(-1),
                step.coasted as u8,
            ));
        }
    }
    if let Some(t) = trace {
        let _ = std::fs::write(t, &log);
    }
    perr.sort_by(|a, b| a.partial_cmp(b).unwrap());
    SimResult {
        omega_end: m.omega,
        sync: commutations as f32 / (m.total / 60.0),
        coast_frac: coasts as f32 / commutations.max(1) as f32,
        post_perr: perr[perr.len() / 2],
    }
}

#[test]
fn cl_sim_locks() {
    let blank = envf("CL_BLANK", 4.0) as u32; // skip the demag tail before the line fit
    let r = run_sim(
        blank,
        Some(&std::env::var("CL_SIM_TRACE").unwrap_or_else(|_| "logs/cl_sim.txt".into())),
    );
    eprintln!(
        "cl_sim_locks: omega->{:.2}, sync {:.2}, coast {:.0}%, post-step period err {:.1}%",
        r.omega_end,
        r.sync,
        r.coast_frac * 100.0,
        r.post_perr * 100.0
    );
    assert!(
        r.omega_end > 1.0 && r.omega_end < 30.0,
        "stalled/runaway: omega={}",
        r.omega_end
    );
    assert!((0.9..=1.1).contains(&r.sync), "out of sync: {:.3}", r.sync);
    assert!(
        r.post_perr < 0.10,
        "period not tracked through the load step: {:.1}%",
        r.post_perr * 100.0
    );
}

#[test]
fn cl_sim_needs_zc() {
    // Negative control: starve the detector (huge blank -> no ZC) so the loop can only
    // dead-reckon. It MUST then fail to track the load step -- proving cl_sim_locks
    // passes because of genuine ZC feedback, not because the rotor is dead-reckonable.
    let r = run_sim(99, None);
    eprintln!(
        "cl_sim_needs_zc (dead-reckon): omega->{:.2}, sync {:.2}, coast {:.0}%, period err {:.1}%",
        r.omega_end,
        r.sync,
        r.coast_frac * 100.0,
        r.post_perr * 100.0
    );
    assert!(
        r.coast_frac > 0.95,
        "expected near-total coasting, got {:.0}%",
        r.coast_frac * 100.0
    );
    let desynced = !(0.9..=1.1).contains(&r.sync) || r.post_perr > 0.10 || r.omega_end < 1.0;
    assert!(
        desynced,
        "dead-reckon should NOT track the step but it stayed locked (sync {:.2}, err {:.1}%)",
        r.sync,
        r.post_perr * 100.0
    );
}
