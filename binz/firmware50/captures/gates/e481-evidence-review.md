E481 is a startup/early-closure tracking stop: reason=8, `from_event=0`, six accepts, seven commutations, 4 ms closed, zero target hold. CCR=133/1333 is approximately 10%, not 25%; the 25% request never reached its dwell.

Recomputed facts:

- Extended age: 4,718,453 − 4,717,426 = **1,027 µs**, exceeding 1,000 µs by **27 µs**.
- Raw separation: 41,405 − 40,367 = **1,038 ticks**, 11 greater than reported age. Under a common µs mapping, the raw acceptance maps to 4,717,415—11 µs before `last_us`. Timestamp semantics need inspection; this alone proves neither corruption nor a wrap bug.
- Sequence: 284,849 − E480’s 284,843 = **6**, consistent with E481’s six accepts and a continuing sequence.

This differs from the described E476 event-origin stop and previous25 LateArm: E481 explicitly reports `from_event=0` and `late_arms=0`. Their underlying captures are absent here.

**What changed next action:** the same image completed E480’s 20.278-second 15% hold, then failed during low-duty entry. Keep the candidate unpromoted; investigate entry timing without retry. At 64 MHz, 133 ticks gives 2.08 µs ON time. Slower entry followed by 48 kHz merits source investigation, not threshold weakening. Filter interaction remains hypothetical; neither current nor supply fault is established.
