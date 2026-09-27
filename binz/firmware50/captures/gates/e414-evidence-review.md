Raw recomputation: 24 = bits 3+4 = CH2N+CH3; 9 = bits 0+3 = CH1+CH2N. Thus **24→24→9→0** matches unchanged staged roles, COM-triggered role transfer, then low pads. The text totals 137 bytes including CRLFs. Reported tests: 372 passed, zero failed.

**No concrete blocker is demonstrated for the proposed exploratory screen**, subject to the stated protection and ownership assumptions. Evidence limits remain:

- Forced-mode, zero-CCR pad checks establish role staging; they do not demonstrate nonzero-PWM compare transfers or exclude switching glitches.
- COMG changes roles before the native update transfers CCRs. A new source can therefore use its inherited compare temporarily. UDIS prevents partial new CCR generations; it does not make roles and compares one atomic generation.
- The model assumes the relevant hardware semantics and deferred-stop behavior. Its enumerated stop positions do not independently validate interrupt timing.
- Neither instruction counts nor the arithmetic audit establish maximum guard delay.
- The e414 pad transcript itself does not identify its firmware hash; its association with the stated installed archive remains asserted.

Retain the conditional 60% step, ≥120 seconds OFF, and failure-ending batch rule. Passing would support these screens only—not qualification or a sag-causality conclusion.
