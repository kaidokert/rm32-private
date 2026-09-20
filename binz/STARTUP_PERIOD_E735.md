# E735 — forced startup compare used the wrong carrier period

Full goal read. E734 progressed by testing level admission, not restoring lock.
Source audit found sixstep::plan fixed compare=6400*duty/1000. Timed20k startup
has TIM1ARR3199, so forced startup61 became compare390/period3200=12.1875%,
not6.1%. Command80 became512/3200=16%. BEMF phase-role path independently
uses correct3200 period; nominal duty labels across transition were inconsistent.
This does not prove the sole cause of missing accepts. Existing sine-period
restore fix E723 did not fix this separate forced-sixstep helper.

Added plan_with_period, validated1..65536 and bounded duty<=100; defaultplan
retains6400 for legacypure callers. Hardware sixstep_write now reads actual
TIM1ARR+1 and uses that period. CCR for61 at3200 is195 (6.09375%). No current,
fault/tracking/watchdog changes. Added module to standalone real policy tests
and exhaustive3200/2667/6400 x sixsteps x duty0..100 assertions and invalidperiods.
31testsPASS,release-s/thinLTO/TIM16helperauditPASS,build/download/reset0.

Installed/root period_735 SHA256:
b277c7f44ad9a9546039164af5c0d46333d786b60e875a89ee18724bca2ba211.
Two normal long62%sine-command (6.2%, units62), later30% requested explorations:
- drive61/phase60: drivenfeedbackstale4at1479us,3scanrecords/1commandchange.
- drive80/phase0: drivenfeedbackstale4at1442us,2scanrecords/1commandchange.
Both no bootstraprelease/BEMFtest, averagecause0, finaloffPASS/Uartclosed.
Captures period_735_start62_drive61_phase60.txt and
period_735_start62_drive80_phase0.txt. Both retained as failed, not envelope gain.

Next explain startup feedback freshness refusal before transfer using producer
timestamps/cadence, not omitted lean summary counters. The wrong compare fix
is valid independently, but these attempts do not demonstrate sustained entry.
Earlier forced-start actual duty must be corrected when interpreting captures;
their shell command labels are not actual20k duty. Goalactive, boardoff.
