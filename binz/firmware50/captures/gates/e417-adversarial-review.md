Raw evidence: 376 tests passed; four ISR arithmetic roots passed; Clippy completed. The incremental-cache warning does not negate those results. None establishes electrical safety or WCET.

**Verdict: a DRIVER-DISABLED boot/pad check is justified, conditional on enforcing driver disable throughout. Powered qualification is unnecessary for this check.**

Concrete blockers if unresolved:

- `board::init` and failure/reset paths are absent: establish ENABLE low before any timer-pad activity, including initialization failure.
- Boot witnesses lack transaction masking and stop checks. Establish that no enabled ISR can compete for TIM1/GPIO or raise ENABLE. Otherwise a stop can interrupt the witness, then foreground `moe_on()` can undo its timer shutdown.
- Successful boot enters `Production::command`. Restrict the test to read-only commands or independently hold driver ENABLE low; this binary is not intrinsically disabled-only.

Runtime COM masking and its active/stopped recheck support transaction exclusion. However, phase 3 unconditionally re-enables EXTI after a possible guard preemption; complete stop dominance is not established. This need not block an isolated disabled witness.

Witness limits: GPIOA/GPIOB reads are sequential, short glitches can escape sampling, and COMG role transfer precedes native-UEV compare transfer. PASS establishes sampled pad patterns only—not glitch freedom, stop latency, powered diode behavior, or the proposed 80% envelope.
