**Must-fix before bounded 50% control**

- **Qualification can accept incomplete carrier evidence.** `applied_period` is optional; the fixture checks a target-derived declaration and CCR arithmetic without requiring actual-period telemetry (`scripts/propless.py:94–108`; `src/run/mod.rs:427–428`). Require actual carrier metadata and establish whether “applied” represents hardware activation or software publication. The controller updates `c.period` immediately after staging, before any confirmed native transfer (`src/run/states.rs:985–992`).

- **Hold timing precedes application.** `hold_start` and current/statistics marks are recorded before retiming/publication, including when `hold_plans` suppresses publication or retiming subsequently refuses (`src/run/states.rs:942–969,983–992`). Consequently, “actual-target dwell” is not established by this timestamp. Qualification must account for publication and activation latency.

- **The implementation proof is incomplete.** The simulator merely logs a successful transaction; it models neither CNT/UDIS nor active/shadow CCRs or COM interleavings (`src/run/sim.rs:387–393`). Establish the invariant across native updates occurring between COM’s mode and compare writes (`src/hw/pwm.rs:89–101`), including the assumption that `table[0].ccr[0]` supplies the correct common compare (`src/roots.rs:1120–1126`). No supplied `sixstep::plan` implementation proves that assumption. Also bound the masked transaction’s guard-stop delay: pending interrupts cannot execute during staging/publication (`src/shared.rs:170–173`; `src/roots.rs:1129–1142`). A pending stop is not yet a latched stop.

**Caveats and findings that are not demonstrated defects**

- UDIS does **not** stop counting. With the stated OCPE configuration, staging preserves the current source’s active compare while updating non-source compares immediately. A COM before overflow can therefore under-drive against the old period; this alone does not demonstrate transitional over-duty (`src/hw/pwm.rs:112–139`). Neither does it prove every interleaving safe.
- Foreground borrowing masks interrupts and releases its borrow before unmasking; COM copies its plan before register writes. No concrete aliasing defect appears here. Retiming never re-enables MOE/ENABLE (`src/shared.rs:166–173`; `src/roots.rs:1234–1236,1117`).
- Boot refusal is fail-closed, but the zero-compare selftest proves only an off-state timing check, not powered CCR coherence. The later PASS message repeats boot status (`bin/order-capture.rs:51–59,74–75`; `src/hw/pwm.rs:145–176`).
- Foldback retention is explicit; restart reset is only asserted with an already-reset input, not demonstrated end-to-end (`src/run/policy.rs:312–323`; `src/run/sim.rs:538–542`).

The supplied evidence does not yet justify 50% control or conditional 60%. Retain every protection.
