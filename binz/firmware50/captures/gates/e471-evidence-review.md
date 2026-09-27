**The snapshot is outside the mask, but the supplied source does not prove that phase‑1 COM can reach that window. Nor does it prove exclusion.** Exact source statements:

- In `roots.rs::det_decide_plain_window`, `let step = Step::new_clamped(S.det().step.load(Ordering::Relaxed) as u8);` precedes `let accepted_arm = W::run(|| ...`. Thus `Masked::run` protects persistence through arm, **not the step snapshot**.
- In `roots.rs::com_root_scheduled`, phase 1 executes `S.det().step.store(step.get() as u32, Ordering::Relaxed);`. If eligible higher-priority COM dispatches between those statements, persistence uses the old local `step` after COM changes both the sector and comparator mux.

Reachability qualifications:

- **Priority:** COM must be able to preempt COMP. Actual `Motor`/`CompPrio` definitions and candidate build features are absent. Comments describing `com-top` are not configuration evidence.
- **Line masking:** COMP enters with `hw::comp::line_disable();`. Its implementation masks **ADC_COMP**, then EXTI18—not TIM16. This alone cannot exclude phase‑1 COM.
- **Phase‑4 park:** COM calls `recheck::defer_for_comp()` before phase dispatch. That delegates to omitted `revisit_delivery::defer(...)`. Its comment promises parking only optional observations; it does not establish phase‑1 exclusion.
- **Handover:** A comment says foreground arms while the detector is already live. The handover implementation is absent, so its actual ordering and exclusion cannot be verified.
- **Stale pending:** COM calls `hw::com_timer::ack();` then dispatches by software `phase`, without a visible expiry check. Whether stale TIM16 pending can survive or arise depends on omitted timer implementations.

**Verdict:** there is a conditional stale-step race; actual candidate reachability remains unknown. The supplied source lacks numeric line labels, so citations above reproduce exact lines rather than inventing numbers.
