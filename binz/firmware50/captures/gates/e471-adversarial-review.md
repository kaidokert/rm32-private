**No concrete stale-snapshot interleaving is established for steady-state operation from the supplied source. Startup remains unproven.** References below identify exact statements because the supplied text has no line numbers.

- **Steady state / previous acceptance:** `det_decide_plain_window` reads `start`, computes `count`, and reads `step` before `W::run`. A higher-priority phase-1 COM between that `step` read and the mask would invalidate the snapshot: COM advances the step and changes the mux (`com_root_scheduled`, phase `1`). However, the previous acceptance leaves COMP masked until that commutation. Its subsequent blank phases `2`/`3` do not change the step. Optional phase `4` cannot advance it either. Merely proposing an outstanding previous phase-1 timer plus an enabled COMP line does not establish reachability.

- **Startup:** `com_arm`’s commentary explicitly describes foreground handover arming while the detector is live. If handover leaves COMP enabled with an outstanding phase-1 timer, and COM has higher priority, the interleaving above is legal. But handover code and the candidate’s actual priority feature configuration are absent. Therefore this is a concrete **conditional counterexample**, not a demonstrated reachable failure.

- **Stale pending:** A pending COMP request alone cannot bypass its NVIC mask (`hw/comp.rs`, `line_disable`). Establishing a counterexample requires showing where that request becomes dispatchable while phase `1` remains outstanding. The supplied startup implementation is insufficient to decide this.

- **Stopped context:** Guard preemption can let COMP resume with old inputs, but `guard_trip` clears activity and latches `com_stop`; `com_arm_crossing` checks `arm_allowed` under exclusion and refuses. This does not recreate timer activity.

**Guard delay:** Yes. A guard request pending but not yet taken when `acceptance::Masked::run` disables interrupts can remain pending through persistence, publication, and arm. There is no pending-guard check; only already-published stop state prevents arming. Nested arm masking preserves the outer mask.
