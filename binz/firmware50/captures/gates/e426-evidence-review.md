No concrete software blocker is established by the supplied evidence; hardware suitability remains unproven.

The entrypoints select identical `DiodeLatched`/`OrderRing` configurations and boot checks; the scheduling policy changes from `PreviousEstimate` to `FreshEstimate`. In the supplied fresh assembly, `0x0800096a` publishes the blended, clamped estimate; `0x0800097a–0x080009a6` derives the wait from that value. `0x080009f8–0x08000a02` rejects oversized or exhausted waits before timer start. The called stop routine’s body is absent, so its shutdown behavior is not independently verified here.

The frozen replay demonstrates the tradeoff: fresh scheduling shortens some descending waits and changes the final rebound from 10 to 12 µs. It cannot establish improved rotor behavior or survival.

Observer cost remains relevant: accepted-row recording occurs after timer start, adding ISR residence beyond the sampled arm expenditure. Its effect on commutation requires interrupt-priority and timing evidence. The reported 778→813 static entries are not WCET; passing tests and the root audit do not establish deadline margin.

The bounded 15% regression, followed by evidence review before any 25% run, remains an experiment—not qualification toward 80% duty.
