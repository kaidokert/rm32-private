# E743 — VREF deployment, measured transition, priorities and foreground cost

Goal read. E742 corrected VDDA conversion. All installed candidates had
release-s/thinLTO/build/download/OpenOCDreset0/TIM16helperauditPASS and own
guard3/18 PASS. All powered tests ended finaloffverified/UARTclosed; no300ACK
or sustained lock. Do not pool these images or infer a hardware ceiling.

- vref_7430707b885d7d5a24f7c5e2988c77415d4b929220ca5b9980ec2b1ad35db95d9f7:
  oldterminalstartup62/61/60/70 stopped sine2372255us, average7610>6923,
  VDDA3313mV nominal800ACK. VREF correction works; no currentgain calibration.
- measured_743b d0715593ede1bed9e2c50496e0f9e5a9173afecb8dbf150b578eb35002bdb4e2:
  removed commanded-bootstrap/polling/revisit, retained startup-level/ADC.
  direct62/61/60/70 timed out40ms with28accepts/48sectors, no six fresh
  consecutive intervals. Averagecause1 reflects guard revocation, not overcurrent.
- compfirst_743c7c76d48cad78edfe3163c04b0b137fd529989f41fb5c6175affca702df6c34e2:
  optionalstartup-comp-top priority0, guard0/DMA40/sector40 retained.
  same test stopped reason15 at700us, oneaccept/11dispatch/maxhandler17us.
  Deadline failed; retired priority0 and restored exact measured_743b.
- measured_743d on same743b: sine62, forced100/phase60, BEMF100. Released
 38540us,40accepts/46commands, measuredseed1565/age290/ARR101/arm7us.
 One BEMF acceptance355us/sector4 then COM2/step5 trackingStale1402us,
 feedback1230us fresh. Avgcause0/residual-12803. Real measured transfer,
 not sustained lock and not repeatability qualification.
- compeer_743e f937b69486004c8c20d7cf5f99da791ffff915fd80e14682ee80fc45c287a6ef:
  restored COM/COMP peer priority as seen in archived E696. Same strong
  transition stopped forceddrive reason15 at5451us/6commands/4accepts,
  before measured handoff. COM powered comparison UNEXERCISED, not falsified.
  This is actual board, OFF/UARTclosed. No further reflash.

Source audit found lean timedstartup still updated ADC row/scan/age stats
inside interrupt::free for every foreground frame, despite E736 guard
publication already moved to DMA. Candidate skips only that diagnostic
work in startupADC+leanIRQ. Keeps original-acquisition LAST_FEEDBACK handoff
token under a short critical section; missingsample still ends13. All real
DMA current/bus/nFAULT/freshness validation remains. No cache timestamp refresh.
Nonlean path retains old rows. Lean scans/age_max now omitted/zero, not absent
feedback evidence. Arbitrary ADC recorder-capacity stop17 is not a lean
control gate. Sine low-rate capture and DMA guard feedback remain available;
startup per-frame row capture is not provided in this lean build.

Candidate root4877be755bbeb90d384d041917bc1e17b0cab3235a7d12dfe83d3c5a144b8dc7
release-s/thinLTO/build0/TIM16auditPASS, NOTflashed. No measured timing gain
claim yet. Next verify shortened foreground masked path and startup scheduling,
then resume practical duty exploration. Recovery/calibrated current/full parity
remain unfinished; flying qualification is not the goal's envelope gate.
