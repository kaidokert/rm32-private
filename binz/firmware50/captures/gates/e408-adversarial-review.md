The experiment is reasonable, but only if “previous estimate” means a coherent pre-accept estimate. It does not reproduce AM32’s COM-time update ordering.

- **Concurrency:** Snapshotting `average_interval` before `offer` introduces a potential stale-read window. Establish exclusive estimator ownership, or capture the preblend value inside the acceptance operation. COM preemption must not mix sector identity, interval history, estimate, or arm state. Verify both plain and logged paths.

- **Stop protection:** E405 must atomically recheck stop/epoch validity when arming; a comparator invocation begun before a stop must never re-enable commutation afterward. Preserve late-arm refusal and every electrical stop. “Same protections” needs verification in the separate binary.

- **Estimator identity:** Require identical state transitions for identical input sequences, including rejection paths, clamping, blanking, watch updates, and publication. Live trajectories will diverge after changing wait; identical observed estimates or blanking durations are therefore not expected.

- **Reference dependency:** AM32 arms using a previously computed wait and updates its estimate at COM. Preblend scheduling approximates that dependency only under established event ordering. During acceleration, the older, larger estimate generally delays COM relative to fresh scheduling; during deceleration, the direction reverses. Check rounding, timer units, `+1`, and helper contracts: the excerpts pass `advance` versus `advance_level` to differently situated `wait_time` functions.

- **Discrimination:** Single50 then conditional60 is a protected screening sequence, not a causal A/B. A pass cannot establish improvement; a failure cannot isolate estimate age. The supplied60 capture has zero target hold. Define the50 pass criterion and log actual ACK/dwell.
