The timestamp correction fixes the identified backdating: `now` is refreshed after observing changed COM count and reused for settling, preventing subtraction against the older `pass()` timestamp. Taking the publication snapshot afterward can miss an intervening COM and delay recognition; that is conservative.

**Concrete stop-dominance gap:** `com_root` checks `active` only at entry. If the guard preempts after that check, suspended COM work can resume after `guard_trip` returns:

- Phase 1 can call `apply_plan` after compares were zeroed.
- Phases 1/2 can reach `comp_exti_arm`; phase 3 can unconditionally re-enable the comparator line.
- `com_arm` refuses subsequent timer arms, but that does not protect these other writes.

Thus “stopped means no further peripheral writes” is false. Actual re-energization is **not established**: it depends on whether those helpers can restore MOE/ENABLE or otherwise defeat their disabled state. Their implementations are absent.

**Staging:** `Seam::lock` masks interrupts across the stop checks, staging, publication and finish. Consequently, the guard cannot interleave within that transaction; staging adds guard-service delay rather than introducing the COM-resumption race above. Preparing the table outside the mask limits that delay. Verifying staging’s claimed register restrictions requires `stage_faster` and `finish_staged_carrier`, also absent.

**Evidence limits / exploration blockers:** The hold test forces count inequality; it does not demonstrate real target-plan application or CCR transfer. The settling bound requires confirming carrier units, preload/update behavior and successful plan application before count increment.

The reported instruction count and 72.6µs estimate neither establish maximum interrupt latency nor prove compliance with the stated 200µs guard gap. Unverified shutdown-preserving helper behavior is a concrete blocker to endorsing powered exploration from these excerpts. Missing WCET proof alone is not evidence of a timing violation; unchanged thresholds and passing host tests do not resolve either question.
