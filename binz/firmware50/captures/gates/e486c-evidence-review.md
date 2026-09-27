43 µs is correct: `ceil(2666/64)+1 = 43`; using 1333 would incorrectly give 22 µs.

`Seam::lock` masks interrupts across owner/MOE checks, staging, table replacement, and UDIS release. Thus guard/COM cannot interleave there. Table construction occurs outside the mask; copying occurs inside. The supplied source does not establish its claimed 320-byte size or execution time. `stage_faster` shows three explicit CCR writes, not a source-level loop.

The COMG model assumes all OCxPE bits already enabled; COMG changes roles, while native update transfers ARR/CCRs. It checks 399→199 at 15% across nine source pairs and 16,384 update masks. It does not prove physical gate timing, establish that initial preload condition, or test guard interruption. The hold test uses Production, not SlowEntry.

No protection-threshold change is evident. Logs report 418 passes and clean audits; they cannot independently verify the supplied SHA.

Conditionally justified as one bounded screen after exact-image flash verification and disabled preflight PASS, honoring stated limits and retaining failures/stopbatch. ≥19 s actual-target dwell requires measurement; energized safety remains unproved.