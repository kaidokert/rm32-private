Corrections address the stale-dispatch setup and add meaningful expiry witnesses. Remaining findings:

- **Case 8 does not isolate the stop latch.** `guard_trip` sets both `active=false` and `stopped=true`. Removing the stopped check from arm admission would still pass. Exercise `stopped=true, active=true` under synthetic, off-only authority, including direct `com_arm` refusal.
- **Case 7 bypasses callback cancellation.** `if case != 7` skips `after_phase`. It tests pending-bit replacement and the new crossing expiry, but cannot demonstrate that an obsolete schedule preserves phase 1 or avoids requesting COMP/rearming.
- **Cancellation is observed after additional cleanup.** The ISR acknowledges UIF and masks TIM16 before checking state; `finish` stops everything again. Case 8 therefore cannot prove the initial stop cleared UIF/UIE or canceled the original deadline permanently. Capture those immediately after the first stop, before reinjection.
- **Case 6 accepts zero requests.** `counts.0 <= 5` allows a completely broken request path to pass. Valid for autonomous observation expiry; insufficient for successful revisit delivery.

No direct energizing operation appears in the supplied probe path. Off readbacks establish sampled state, not continuous absence of gate activity. With corrected claims, one `u` suite is useful evidence, but not acceptance of latch isolation or obsolete-callback cancellation.
