# E767 — normal restart truth and remaining failure

The board ADC is now the routine voltage/current instrument. At the retained
PSU anchor the current estimate was about 11% conservative; carry 10–15%
uncertainty and request another PSU reading only for disagreement, saturation,
nonlinear behavior or a genuinely new operating regime.

## Verifier correction

`NORMALRESTART result=1` records that firmware launched the second ordinary
startup. It does not prove that startup reached powered BEMF control or stayed
there. `scripts/live_armed_baseline.py` now requires all of:

- the original stop is tracking reason8 and the one-shot uses no flying seed;
- a fresh `DRIVEX result=1 power_reason=2` transfer;
- one authoritative `POWERPATH reason=2` at the retained remaining deadline;
- final outputs-off validation.

The foreground `COASTREF` observer may report stop1 or stop7 depending on which
owner observes the same deadline first. `POWERPATH` remains authoritative.
Tests accept the genuine retained E765 pass and reject both E765 false passes.

## Count-cap correction

The lean E765 feature set still enforced 64 comparator dispatches per 1 ms as
a hard acquisition stop. E767 keeps that peak as telemetry but stops only for
the measured 50 us handler-overrun bound and the independent command/feedback
watchdogs. The special rate-snapshot diagnostic retains the old forced refusal.

Frozen E767 ELF:
`captures/reference/m0clean_767/shell-pwm.elf`

SHA256:
`0B0C180FC0CFE6D6BF02759C674D96D4BB8BF208BB1211B6BC005A4BCF4CA82F`

It uses release `opt-level=s`, thin LTO and one codegen unit. The mandatory
link audit found no forbidden division/remainder, 64/128-bit or float helper
reachable from DMA1, ADC_COMP, TIM16 or TIM6. Disabled guard preflight passed
3 timer-fault cases, 18 post-stop refusals and final outputs-off.

## Powered result

The predeclared actual restart attempts were:

| capture | acquisition | powered outcome |
|---|---|---|
| `..._02` | peak71/ms, max22us, overruns0 | second sine startup stopped on average/current reason4 before a second powered handoff; printed power reason8 was stale first-segment state |
| `..._03` | peak52/ms, max22us, overruns0 | average-current reason25 at2116us |
| `..._04` | peak65/ms, max22us, overruns0 | deadline reason2 at22,980,005us |

Result: **1/3**, not qualified. The pass above the former cap proves the cap
change was useful; the two current-related failures prove it was not the whole
problem. Current and tracking protection remain unchanged. Later forensic
review corrected the original run02 attribution: it was not a powered tracking
loss. The second ordinary sine startup stopped at about 4.056 s, before the
second BEMF handoff, and the printed reason8 belonged to the retained first
segment.
