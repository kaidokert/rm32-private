Raw-source recomputation: average 80 gives deadlines 41, 121, 161, 201, 241 µs. At age 60 plus handshake time, one observation should consume a slot and rearm toward 121 µs, provided service remains earlier than that deadline. Cases 10/11 correctly distinguish native expiry (`UIF=1`) from software dispatch after stop (`UIF=0`).

Two evidence weaknesses need fixing:

- **Case 12’s stop assertion is contaminated by cleanup.** `finish()` calls `guard_trip(HostAbort)`, and foreground repeats it before checking `stopped`. Thus `stopped=true` cannot prove the overrun path stopped anything. Capture stopped/CEN/active immediately after `resume_after_refusal()`, before cleanup; assert overrun latched and timer stopped there.
- **Acceptance cancellation coverage is incomplete.** Case 10 parks an already-consumed expiry, then arms directly without publishing a new generation or creating a pending refusal wake. It cannot establish cancellation of an outstanding wake or generation invalidation. Add that adversarial ordering explicitly.

No direct bridge-enable path appears in the supplied suite. However, actual priority initialization depends on omitted `board::init`; verify COM=0x40 and COMP=0x80 before flashing. Source assertions are not execution evidence.
