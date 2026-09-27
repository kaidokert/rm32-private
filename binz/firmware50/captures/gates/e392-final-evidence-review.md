The timestamp correction fixes the shown backdating bug: `hal.now()` is sampled after observing COM, and replaces `now` before settle arithmetic. On that poll, `now.wrapping_sub(at)` is zero, not an unsigned underflow. Subsequent clock rollover works provided elapsed time is less than one full `u32` clock period.

**Remaining defects**

- **Target evidence survives a downward duty change.** Lines 1014–1017 reset COM/settle evidence only for a new duty at or above target. If duty falls below target before hold starts, stale evidence remains. More directly, hold marking precedes applying this poll’s computed duty: it can mark a hold using the old target duty immediately before publishing a lower duty. Eligibility should include the pending duty; below-target publication should invalidate pending target evidence.
- **Stopping the timer does not cancel an executing COM handler.** `com_root` checks `active` only at entry. If a guard can preempt after that check, it can complete shutdown before COM resumes into `apply_plan`, `comp_exti_arm`, or `line_enable`. `com_arm` correctly refuses rearming, but those other operations lack an equivalent transactional check. This permits post-stop writes/unmasking; the excerpts do **not** establish bridge re-energization. Whether that interleaving is reachable depends on actual IRQ priorities.

**Caveats**

- The retime transaction shown is interrupt-masked through `Seam::lock`, including stop checks, staging, publication, and finish. A maskable guard cannot split that transaction.
- The new test exercises fresh timestamp use and immediate settle rejection, but fabricates COM observation by altering the saved count. It does not test actual COM/preload ordering, exact settle boundaries, clock rollover, downward-duty invalidation, or stop preemption.
- Reported test totals and instruction estimates do not establish WCET or electrical qualification.
