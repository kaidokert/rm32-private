//! Retained physical capture regression: replay must stop at drive/mux mismatch.
use std::{path::PathBuf, process::Command};

#[test]
fn recorded_levels_cannot_follow_counterfactual_commutation() {
    let capture=PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../captures/bemf_irqwave_spin_01_obs.csv");
    let result=Command::new(env!("CARGO_BIN_EXE_drv-observer-replay"))
        .args(["--full-polling","6667"]).arg(capture).output().unwrap();
    assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    let out=String::from_utf8(result.stdout).unwrap();
    assert!(out.contains("REFERENCE_COM us=2973.5 step=2"),"{out}");
    assert!(out.contains("DIVERGENCE us=3112 model_step=2 recorded_step=1"),"{out}");
    assert!(out.contains("stop=physical_step_or_mux_divergence lock_proven=0"),"{out}");
    assert_eq!(out.matches("REFERENCE_COM").count(),1);
}
