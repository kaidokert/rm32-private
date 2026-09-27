Both runs stopped on faults; neither demonstrates successful completion.

New versus control: target hold 3,729 versus 2,669 ms (+1,060 ms, +39.7%); accepted events 167,559 versus 147,759 (+13.4%); hold acceptance rate approximately 18,230 versus 18,106/s (+0.69%). Coast estimates increased 2,930→3,049 eHz (+4.1%). Current proxies increased 424→455 mA (+7.3%); these are signed block measurements, not calibrated supply current or peaks.

Control tracking arithmetic checks: 14,884,609−14,884,357=252 µs, exceeding the reported 240 µs limit by 12 µs. This establishes the recorded stale-reference trip, not its cause.

New reports guard 15 (LateArm), one late arm, and 10 µs spent at CI=41 µs. A 10 µs wait gives zero remaining time; however, the supplied code omits the CI/advance-to-wait calculation. DONE 28 denotes UnknownGuard, consistent with a reporting/decode problem, not success.

The patch adds masked sector/phase revalidation; it cannot distinguish six-commutation recurrence. One run per image cannot establish causal attribution or reliability improvement. Next, verify the actual wait calculation and address the demonstrated deadline exhaustion while preserving protections.
