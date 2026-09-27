Raw evidence: 376 tests passed; the incremental-cache warning does not negate that result. Clippy completed, and the supplied audit reports four clean ISR roots. No actual pad-test result is supplied.

Source and assembly agree: the masked transaction rechecks active/stopped, sets UDIS/CCPC, writes preloaded CCRs, clears the source’s CCxNE, issues COMG, then restores CR2/CR1 and PRIMASK. It does not write TIM1 CNT, UG, or MOE. Roles and compares still transfer at different events; this is not simultaneous role/CCR publication.

396/1333 = 29.707%; 64 MHz/1333 ≈ 48.012 kHz, conditional on the timer configuration. The supplied excerpts do not independently establish the inherited maximum or 80% cap. Executed-instruction count is not WCET.

**The driver-disabled boot/pad check is justified; no concrete blocker is demonstrated.** Confirm that `enable_is_high()` reads the actual ENABLE pad and that initialization/interrupts cannot raise ENABLE during the witness; those implementations are omitted.

The subsequent `Production` command loop permits operation beyond this test’s scope, so restrict the session to disabled inspection. Its `p` message reports historical boot success, not a fresh waveform check. GPIOA/GPIOB samples are sequential and cannot exclude short glitches. None of these limitations requires powered qualification before this disabled test.
