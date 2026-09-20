# E737 — initial BEMF duty and comparator GPIO audit

Goal attachment read. Current board/root remains E736c SHA256
18FA024E4292FD713A0AA643CD61CA8FF6F837C030E0D50A8040B389F0603EB7.
No firmware flash or protection change in this experiment.

The terminal fixture now exposes `--bemf-duty` (40..100 tenths percent,
default70) and requires the corresponding initial-duty query ACK before
live steps. This is the initial BEMF setting, not a new live throttle cap.

Retained capture: `captures/bemfduty_737_start62_drive80_bemf100.txt`.
Command: live_armed_baseline.py with --ms30000 --ramp-duty300 (passed as
separate arguments), --nominal-average --lean-explore --cycle450
--fast-cycle-report --event100 --dma-guard --seed500 --direct-start
--startup-duty62 --drive-duty80 --drive-phase0 --bemf-duty100.

Initial BEMF100 ACK received. Driven release22 at7873us, six accepted
inputs/nine commands. BEMF COM2/physicalstep5, last accepted event342us,
sector4; last poll1402us, feedback1242us (160us old). Tracking Stale
stopped at1405us. Average diagnostic cause0. Final off verified/UARTclosed.
Fixture exit0 records a completed exploration, not success:
target_acknowledged=False, no sustained BEMF or 30% result.
Widening initial PWM duty from7% to10% did not remove the missing sector5
accept. Do not infer a hardware ceiling or qualify this build from the test.

Source audit: PA2/PA3/PB3/PB7 initialized analog. Live role MODER writes
and disabled AF restoration mask only PA7..10/PB0..1. A pure regression
test covers all six roles and AF restoration preserving comparator analog
fields. This rules out these specific mode writes clobbering the sensing
pins, not every possible register/configuration fault. No wiring change.

Verification: rustc host harness scripts/live_role_tests.rs, 21/21 PASS;
Python test_drv_capture.py, 8/8 PASS. The new Rust test is test-only and
adds no firmware runtime work. No firmware qualification is implied.

Next: restore sustained-running capability by isolating the startup/handoff
regression against the previously working path, not another blind duty sweep.
