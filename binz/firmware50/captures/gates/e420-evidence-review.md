Raw evidence supports a completed 15% exploration, not qualification:

- Target hold: 20.278s, below 30s.
- Hold rate: 266314/20.278 = 13,133.54 accepts/s.
- Tail: 2,278,019−90 = 2,277,929µs; 30,163 accepts imply 2,206.90 electrical Hz.
- Tail age: 90µs ≤ 240+104+217 = 561µs.
- CCR: floor(1333×150/1000)=199, matching capture.
- The 32 coast intervals sum to 7,280µs: their simple mean implies 2,197.80Hz. This differs from the reported time-anchored 2,207Hz; the missing reconstruction algorithm prevents verifying that result.
- Both safe-off records match; reported forced commutations and checked fault flags are zero.

**End-to-end host admission is not established.** `cohort.fields`, `parse`, and `run_gates` are absent, so neither matched-window identity nor inherited safeguards can be independently verified. Test names and “OK” do not supply their implementations.

A concrete admission gap exists: the supplied pre-hardware checks require a carrier argument but do not restrict it to 1000/1333; that rejection occurs only in post-capture `verdict`.

Command8/duty250/no pre-keys/no override passes the shown request validator. However, no verified mechanical-speed limit supports assessing the proposed increase. Cooling bounds do not resolve that missing condition. The digital models also provide no analog transient proof.
