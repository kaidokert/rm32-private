Prioritize a stop/re-enable race over another instruction shave.

The timer arm is protected: `0x08000960–0x08000a32` masks interrupts across validation, preparation, expiry handling and enable. Comparator re-enabling lacks equivalent protection. A guard trip can preempt a refused COMP decision, mask the line and stop COM; resumed COMP then executes `line_enable()` at `0x08000b9e–0x08000baa`. COM’s phase-2/3 paths similarly lack a final atomic stop check. This establishes possible interrupt re-enablement after shutdown, **not bridge re-energization**.

Latency evidence is narrower than the optimization objective:

- `raw` is sampled at `0x08000624`, after entry, masking and acknowledgement; interrupt dispatch delay is excluded.
- `spent` ends at `0x08000994`, before deadline arithmetic and timer enable at `0x08000a2e`.
- `call_max_us` excludes the final re-enable/return and bypassed paths. Peer-priority COM can wait behind COMP’s post-arm bookkeeping and guard processing.
- At fixed16, `ci=41` gives `wait=(41>>1)-(41>>2)=10`; recorded `spent_at_late=10` explains the software expiry decision. It establishes neither physical causation nor an end-to-end latency bound.

The emitted path already uses shifts for fixed advance and blanking. Shortening persistence instructions changes sampling spacing; removing masks changes concurrency. This single capture cannot establish a meaningful safe arithmetic saving.

**One bounded next change:** route closed-loop comparator re-enables through a shared critical section that rechecks `com.active && !com.stopped` before enabling. Preserve every existing guard and timer transaction. Verify guard-preemption interleavings immediately before that section cannot reopen the line. This closes a demonstrated race; latency improvement remains unproven.
