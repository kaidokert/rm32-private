//! Host capture-replay harness for the OBSERVE-ONLY closed-loop tracker (Stage 1b).
//!
//! Runs `rinz::cl::ClTracker` over captured BEMF frames exactly as the firmware ISR
//! would — causal, frame-by-frame — so what we validate on host IS the firmware code.
//! Reads the frames file from `cl_export.py` (env CL_FRAMES), pushes each frame, fires
//! a commutation at every physical-sector boundary, and writes the per-commutation
//! trace (env CL_TRACE) for `cl_score.py` to compare against the oracle.
//!
//! Tracker params come from the environment so they can be swept without recompiling:
//!   CL_KP CL_KI CL_GATE CL_BLANK  (e.g. `CL_KP=0.2 CL_GATE=20 cargo cl-replay`)
//!
//! Run via `cargo cl-replay` (alias in .cargo/config.toml).

use rinz::cl::ClTracker;

fn env_f(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
fn cl_replay() {
    let frames_path = std::env::var("CL_FRAMES").unwrap_or_else(|_| "logs/cl_frames.txt".into());
    let trace_path = std::env::var("CL_TRACE").unwrap_or_else(|_| "logs/cl_trace.txt".into());
    let kp = env_f("CL_KP", 0.3);
    let ki = env_f("CL_KI", 0.2);
    let gate = env_f("CL_GATE", 25.0);
    let blank = env_f("CL_BLANK", 2.0) as u32;

    let text = std::fs::read_to_string(&frames_path)
        .unwrap_or_else(|e| panic!("read {frames_path}: {e} (run cl_export.py first)"));

    let mut out = String::from("# cap comm phys zc_raw zc_track accepted oracle\n");
    let mut lines = text.lines();
    let mut n_caps = 0u32;

    while let Some(line) = lines.next() {
        if !line.starts_with("CAP") {
            continue;
        }
        let (mut hz, mut shz, mut nfr, mut idx) = (0.0f32, 0.0f32, 0usize, 0usize);
        for kv in line.split_whitespace().skip(1) {
            let mut it = kv.split('=');
            match (it.next(), it.next()) {
                (Some("idx"), Some(v)) => idx = v.parse().unwrap_or(0),
                (Some("hz"), Some(v)) => hz = v.parse().unwrap_or(0.0),
                (Some("sample_hz"), Some(v)) => shz = v.parse().unwrap_or(0.0),
                (Some("frames"), Some(v)) => nfr = v.parse().unwrap_or(0),
                _ => {}
            }
        }
        let oracle: Vec<f32> = lines
            .next()
            .unwrap_or("")
            .split_whitespace()
            .skip(1)
            .map(|s| s.parse().unwrap_or(-1.0))
            .collect();
        if hz <= 0.0 || shz <= 0.0 || nfr == 0 {
            continue;
        }
        let fps = shz / (hz * 6.0);
        let mut tr = ClTracker::new(fps, kp, ki, gate, blank);
        let mut prev: Option<usize> = None;
        let mut comm = 0usize;
        for i in 0..nfr {
            let fl = lines.next().unwrap_or("");
            let mut p = fl.split_whitespace();
            let a = p.next().and_then(|x| x.parse().ok()).unwrap_or(0i32);
            let b = p.next().and_then(|x| x.parse().ok()).unwrap_or(0i32);
            let c = p.next().and_then(|x| x.parse().ok()).unwrap_or(0i32);
            let sector = ((i as f32 / fps) as usize) % 6;
            if let Some(ps) = prev {
                if sector != ps {
                    let s = tr.commutate(ps as u8);
                    let raw = match s.zc_raw {
                        Some(z) => format!("{z:.1}"),
                        None => "-1".into(),
                    };
                    let o = oracle.get(ps).copied().unwrap_or(-1.0);
                    out.push_str(&format!(
                        "{idx} {comm} {ps} {raw} {:.1} {} {o:.1}\n",
                        s.zc_track, s.accepted as u8
                    ));
                    comm += 1;
                }
            }
            tr.push_frame([a, b, c], sector);
            prev = Some(sector);
        }
        n_caps += 1;
    }

    std::fs::write(&trace_path, out).unwrap_or_else(|e| panic!("write {trace_path}: {e}"));
    eprintln!("cl_replay: {n_caps} captures -> {trace_path}");
}
