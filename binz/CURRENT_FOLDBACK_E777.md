# E777 — foreground average-current foldback

## Result

The opt-in exploration image now treats the first complete signed-average
over-current block as a warning and the second consecutive over-current block
as the existing hard stop. Foreground consumes the warning and lowers the
coherent live PWM command by exactly 5 percentage points. The resulting duty
is a monotonic ceiling for the rest of the powered session: later host commands
and ordinary-restart restoration cannot raise it again.

The DMA ISR still owns only scan validation, accumulation, the ratings-derived
same-sample clamp, and warning publication. It performs no PWM write, division,
or foldback arithmetic. Bus undervoltage, nFAULT, tracking loss, handler/runaway
watchdogs, ADC rails, and the second-block average-current stop are unchanged.

Installed and frozen ELF SHA256:
`919BEB3C8AE1E13C1B0A8CA489FB931FBE6286206DA009C0EB3245EC32D226BB`

Build profile is release `opt-level=s`, thin LTO, one codegen unit. The emitted
direct-call audit reports zero forbidden arithmetic helpers reachable from
DMA1, ADC/COMP, TIM16, or TIM6. The disabled preflight passed 3 timer-fault
cases and 18 post-stop write refusals.

## Powered evidence at the operator's 1.5 A PSU ceiling

The threshold remains a nominal 1.5 A signed bridge-return estimate, not a new
precision calibration. The prior PSU anchor says the board estimate is roughly
10–15% conservative; onboard current and bus channels remain the routine
instruments.

1. `currentfold_777_ramp300_30s_02.txt`: live 7→10→15→20→25→30% ramp,
   full 30 s powered hold, reason 2 deadline, `CURRENTFOLDBACK count=0`, bus
   minimum 11.689 V, 219,795 commutations, commit maximum 8 us, final outputs
   off and nFAULT high.
2. `currentfold_777_tracktrip_restart300_30s_01.txt`: 30% requested and
   acknowledged; injected tracking decision stopped at 5.000052 s; one-second
   disabled settle; ordinary startup and fresh BEMF handoff completed. After
   restoration reached 30%, the first over-current block caused exactly one
   30→25% foreground foldback. The run then completed its original remaining
   19.238005 s deadline with reason 2, bus minimum 11.760 V, no electrical
   fault, and final outputs off. This directly replaces the E775 run03 behavior
   where the same regime ended in reason 25.

This is functional exploration evidence, not a 30% recovery cohort or a
calibrated-current qualification. The first host attempt was cut at the tool's
30 s boundary before its capture was written; it was immediately followed by
a fresh disabled guard/off check and is not counted.

## Host verification

The fixture now has an explicit `--current-foldback` verifier. It still demands
the original high-duty acknowledgment, fresh normal restart, original deadline,
and final safing; only an exact `CURRENTFOLDBACK` record may explain a lower
effective restoration ceiling. Without that record the historical zero-step
restoration refusal remains intact.
