Independent arithmetic: the immediate-write test covers 12 directed adjacent transitions × 2 PWM levels × 3 write boundaries = **72 sampled states**, with 216 channel checks. The latch test covers 36 ordered sector pairs × 2 levels = **72 cases**, including self-transitions.

The reported **10 intermediate-low observations and 2 double-high observations cannot be independently recomputed without `plan()` or its six register images**. These counters count observations, potentially repeating a condition across write boundaries; they do not count distinct glitches.

Coverage limitations:

- Same-leg exclusion follows algebraically from `(reference, enabled && !reference)`; its assertion cannot establish hardware shoot-through safety.
- A shared, fixed `pwm_high` omits CCR differences, carrier progression, update boundaries, dead time and actual pulse suppression.
- The latch test assumes atomicity through `active = preload`. It checks the proposed abstraction, not whether target configuration implements it.
- “Interrupted staging” never actually executes a stop or competing register write. The MOE assertion is tautological because `moe` is assigned `false`.
- The safe-off recorder verifies call order; sequential repetition does not test reentrancy or physical output behavior.

**Offline implementation is reasonable, but stop interleaving is a concrete unresolved requirement.** A guard preempting before the remaining CCR stores can zero compares, then resumed COM can overwrite those zeros. COMG not setting MOE preserves that particular shutdown barrier, but does not preserve the zero-compare postcondition.

Audit every preemption boundary, actual `Bridge` implementation, restart ownership, and CCPC cleanup. The supplied evidence supports investigating torn role updates; it does not establish measured pulses, sag causation, or motor admission.
