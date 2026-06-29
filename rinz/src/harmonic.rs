//! Streaming harmonic ZC detector — the firmware form of the validated batch oracle.
//!
//! Per phase, fit `e = a*cos(t) + b*sin(t) + c` to the floating-window BEMF via RUNNING,
//! exponentially-decaying normal-equation sums updated every tick (uniform per-tick cost —
//! the constant-cost ISR constraint), solved per commutation. Unlike the per-sector
//! sign-change/linfit, this persists across sectors and pools each phase's float arcs over
//! ~1 recent electrical rev, so it recovers the OUT-OF-WINDOW crossings (the ~70% the loop
//! currently coasts) and stays viable at high speed (it has ~12 samples/rev/phase even at
//! 1300 Hz, vs ~2/sector for the per-sector detectors).
//!
//! Host-validated (`scripts/harmonic_stream.py`): the streaming fit tracks the batch oracle
//! to 2.7° worst-case across 250–400 Hz. `cos/sin` per tick come from an incremental rotation
//! in the caller (trig-free); only the per-commutation crossing solve uses trig (micromath).

use micromath::F32Ext;

const TWO_PI: f32 = core::f32::consts::TAU;

/// Running per-phase a*cos+b*sin+c via decaying normal-equation sums.
pub struct Harmonic {
    lam: f32,
    // per phase: [Scc, Scs, Sc, Sss, Ss, S1, Svc, Svs, Sv]
    s: [[f32; 9]; 3],
}

impl Harmonic {
    /// `lam` = per-sample decay (0.92 ≈ 1/e over ~12 float samples ≈ 1 rev/phase at our rate).
    pub const fn new(lam: f32) -> Self {
        Self {
            lam,
            s: [[0.0; 9]; 3],
        }
    }

    pub fn reset(&mut self) {
        self.s = [[0.0; 9]; 3];
    }

    pub fn set_lam(&mut self, lam: f32) {
        self.lam = lam;
    }

    /// Push one float-window sample for `phase` at electrical angle whose cos/sin are given
    /// (caller maintains them by incremental rotation — no per-tick trig). Decays then adds.
    pub fn push(&mut self, phase: usize, cos_t: f32, sin_t: f32, e: f32) {
        let s = &mut self.s[phase % 3];
        for v in s.iter_mut() {
            *v *= self.lam;
        }
        s[0] += cos_t * cos_t;
        s[1] += cos_t * sin_t;
        s[2] += cos_t;
        s[3] += sin_t * sin_t;
        s[4] += sin_t;
        s[5] += 1.0;
        s[6] += e * cos_t;
        s[7] += e * sin_t;
        s[8] += e;
    }

    /// Solve the 3×3 normal equations for `(a, b, c)`. None if under-sampled or singular.
    pub fn solve(&self, phase: usize) -> Option<(f32, f32, f32)> {
        let s = &self.s[phase % 3];
        if s[5] < 6.0 {
            return None; // not enough effective samples yet
        }
        // Symmetric normal matrix M and rhs.
        let m = [[s[0], s[1], s[2]], [s[1], s[3], s[4]], [s[2], s[4], s[5]]];
        let rhs = [s[6], s[7], s[8]];
        let det = det3(&m);
        if det.abs() < 1e-6 {
            return None;
        }
        let a = det3(&replace_col(&m, 0, &rhs)) / det;
        let b = det3(&replace_col(&m, 1, &rhs)) / det;
        let c = det3(&replace_col(&m, 2, &rhs)) / det;
        Some((a, b, c))
    }

    /// Electrical angle (rad, [0,2π)) of the `rising`/falling zero crossing of the fitted
    /// sinusoid, or None if the fit doesn't cross (|c| > amplitude) or is degenerate.
    pub fn cross(&self, phase: usize, rising: bool) -> Option<f32> {
        let (a, b, c) = self.solve(phase)?;
        let r = (a * a + b * b).sqrt();
        if r < 1e-6 || c.abs() > r {
            return None;
        }
        // a*cos t + b*sin t = r*sin(t + psi); zero at sin(t+psi) = -c/r.
        let psi = a.atan2(b);
        let base = asin_safe(-c / r);
        for &shift in &[base, core::f32::consts::PI - base] {
            let t = wrap_2pi(shift - psi);
            let deriv = -a * t.sin() + b * t.cos();
            if (deriv > 0.0) == rising {
                return Some(t);
            }
        }
        None
    }

    /// The fitted zero crossing (rad) NEAREST `target` (circular distance), either edge, or
    /// None. Used to pick the crossing belonging to a given sector's float window — each phase
    /// crosses twice per rev (once per its two float sectors); we want the one near this sector.
    pub fn cross_near(&self, phase: usize, target: f32) -> Option<f32> {
        let (a, b, c) = self.solve(phase)?;
        let r = (a * a + b * b).sqrt();
        if r < 1e-6 || c.abs() > r {
            return None;
        }
        let psi = a.atan2(b);
        let base = asin_safe(-c / r);
        let mut best: Option<f32> = None;
        let mut best_d = f32::MAX;
        for &shift in &[base, core::f32::consts::PI - base] {
            let t = wrap_2pi(shift - psi);
            let d = ang_dist(t, target);
            if d < best_d {
                best_d = d;
                best = Some(t);
            }
        }
        best
    }

    /// Effective (decayed) sample count for `phase` — a confidence proxy for the fit.
    pub fn weight(&self, phase: usize) -> f32 {
        self.s[phase % 3][5]
    }
}

/// Smallest absolute circular distance between two angles (rad), in [0, π].
pub fn ang_dist(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % TWO_PI;
    if d < 0.0 {
        d += TWO_PI;
    }
    if d > core::f32::consts::PI {
        TWO_PI - d
    } else {
        d
    }
}

/// Signed (a - b) wrapped to (-π, π].
pub fn wrap_pm_pi(x: f32) -> f32 {
    let mut d = x % TWO_PI;
    if d <= -core::f32::consts::PI {
        d += TWO_PI;
    }
    if d > core::f32::consts::PI {
        d -= TWO_PI;
    }
    d
}

fn det3(m: &[[f32; 3]; 3]) -> f32 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

fn replace_col(m: &[[f32; 3]; 3], col: usize, v: &[f32; 3]) -> [[f32; 3]; 3] {
    let mut out = *m;
    for r in 0..3 {
        out[r][col] = v[r];
    }
    out
}

/// asin via a polynomial (Abramowitz-Stegun 4.4.45, ~1e-4 rad) -- NO atan2. The crossing solve
/// runs per commutation in the ISR and atan2 is ~450 cyc on the M4; this keeps it to 1 sqrt +
/// a few muls. Accuracy is ample for a ZC angle.
fn asin_safe(x: f32) -> f32 {
    let x = x.clamp(-1.0, 1.0);
    let neg = x < 0.0;
    let a = if neg { -x } else { x };
    let poly = 1.570_796_3 - a * (0.214_512_4 - a * (0.087_417_5 - a * 0.044_894_2));
    let r = core::f32::consts::FRAC_PI_2 - (1.0 - a).sqrt() * poly;
    if neg { -r } else { r }
}

fn wrap_2pi(mut t: f32) -> f32 {
    while t < 0.0 {
        t += TWO_PI;
    }
    while t >= TWO_PI {
        t -= TWO_PI;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use micromath::F32Ext;

    #[test]
    fn recovers_known_sinusoid() {
        // e = 100 cos t + 50 sin t + 10, sampled over a full period -> solve should recover it.
        let mut h = Harmonic::new(1.0); // no decay for a clean fit
        for k in 0..24 {
            let t = k as f32 / 24.0 * TWO_PI;
            let e = 100.0 * t.cos() + 50.0 * t.sin() + 10.0;
            h.push(0, t.cos(), t.sin(), e);
        }
        let (a, b, c) = h.solve(0).unwrap();
        assert!((a - 100.0).abs() < 0.5, "a={a}");
        assert!((b - 50.0).abs() < 0.5, "b={b}");
        assert!((c - 10.0).abs() < 0.5, "c={c}");
    }

    #[test]
    fn finds_the_crossing() {
        // Pure sin (a=0,b=1,c=0): rising zero at t=0 (and 2pi), falling at t=pi.
        let mut h = Harmonic::new(1.0);
        for k in 0..24 {
            let t = k as f32 / 24.0 * TWO_PI;
            h.push(1, t.cos(), t.sin(), t.sin());
        }
        let fall = h.cross(1, false).unwrap();
        assert!((fall - core::f32::consts::PI).abs() < 0.05, "fall={fall}");
        let rise = h.cross(1, true).unwrap();
        // rising crossing is at 0 == 2pi; accept either end
        assert!(rise < 0.05 || (rise - TWO_PI).abs() < 0.05, "rise={rise}");
    }

    #[test]
    fn none_when_offset_exceeds_amplitude() {
        // c far above the amplitude -> never crosses zero.
        let mut h = Harmonic::new(1.0);
        for k in 0..24 {
            let t = k as f32 / 24.0 * TWO_PI;
            h.push(2, t.cos(), t.sin(), 10.0 * t.cos() + 500.0);
        }
        assert!(h.cross(2, false).is_none());
    }

    #[test]
    fn decay_tracks_a_shift() {
        // After the input shifts, a decaying fit should follow it (not stay stuck on history).
        let mut h = Harmonic::new(0.85);
        for k in 0..60 {
            let t = k as f32 / 24.0 * TWO_PI;
            h.push(0, t.cos(), t.sin(), 100.0 * t.cos()); // c=0 -> crossing at pi/2 (fall)
        }
        // shift offset up so crossing should disappear / move; feed enough to decay history
        for k in 60..120 {
            let t = k as f32 / 24.0 * TWO_PI;
            h.push(0, t.cos(), t.sin(), 100.0 * t.cos() + 30.0);
        }
        let (_, _, c) = h.solve(0).unwrap();
        assert!(c > 15.0, "decayed fit should reflect the new offset, c={c}");
    }
}
