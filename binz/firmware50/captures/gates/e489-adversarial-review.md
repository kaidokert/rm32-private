Raw evidence: CCR 319 / period 1333 = **23.93% applied duty**, versus **25.0% requested**. Target hold was **0 ms**, with zero hold accepts and no tail samples; this was not a completed target-hold result.

Stop reason **28** accompanies guard/late-arm subtype **15**: one late arm recorded **ci=44 µs, spent=11 µs**. At advance16, **wait=ci/4=11 µs**, so measured work consumed the entire wait budget. The histogram counters are unpopulated, so they cannot establish margin probabilities.

The terminal ci is not a mean-speed measurement. Coast reports about **2930 eHz** separately. Zero forced commutations do not establish reliable operation.

Preflight and post-stop checks report gates disabled; the subsequent self-test passes. These checks do not negate the runtime late arm.

**Verdict:** a captured timing-margin failure with no target-hold validation. This single run establishes no false-edge, supply, carrier, or reliability attribution. Honor the operator’s pause; no further bench admission.