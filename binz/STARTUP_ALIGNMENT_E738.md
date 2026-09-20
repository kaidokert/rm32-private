# E738 — startup alignment versus increased startup duty

Full goal read. Previous turn progressed by GPIO audit/test. No firmware
flash or protection change in either powered test. Actual board remains
E736c18FA024E4292FD713A0AA643CD61CA8FF6F837C030E0D50A8040B389F0603EB7.

Both tests used live_armed_baseline.py, 30s requested window, ramp target300,
nominal-average, lean-explore, cycle450/fast-cycle-report/event100/dma-guard,
seed500/direct-start, initial BEMF100, phase60. Final off verified and UART
closed for both. Neither acknowledged target300; exit0 is not a motor pass.

1. alignment_738_start62_drive61_phase60_bemf100.txt:
   BEMF timer armed, COM1/step5, no powered accepted event; trackingStale
   at1002us, feedback756us (246usold). Startup DI85 records eight accepted
   events from commanded sectors3/4/6 only across epochs3..22. These are
   sparse acceptances, not evidence of continuous six-sector startup tracking.
   Startup rate_peak83 is a report, not this stop's cause. Avgcause0.
2. starttorque_738_start100_drive100_phase60_bemf100.txt:
   energized1115291us, STARTUPADCfault25, averagecause3, residual3889 versus
   allowance3185; stopped during sine startup before forced drive/handoff.
   Not a torque or BEMF verdict. This nominal current setting is uncalibrated;
   do not turn raw residual into amperes or infer a real PSU overload.

Source audit found no concrete owner/mask defect. Changed foreground
post-stop CORESEED provenance: commanded startup now assumed1/source
commanded_startup; genuine measured flying remains assumed0/measured_flying.
No new ISR bookkeeping. Candidate source differs from installed E736c;
Release-s/thinLTO build exit0 and TIM16 helper audit PASS. Candidate SHA256
bf6040c53f76824e372c2939bfd7e7312cf7e33844d475136307a25fb9b2f43e,
NOT flashed. Audit is not a timing or full transitive arithmetic proof.
This fixes reporting, not the motor.

Next isolate why startup yields sparse sector acceptances, preferably against
known working startup/control evidence. Flat10% startup is not a usable fix
with the current average policy. No new hardware-limitation claim.
