# E750: compare historical seed criterion, retain unsuccessful intervention

Goal read. Historical E696 sustained20%/940eHz build used twelve measured
intervals; timed startup recently selected with_startup_cycle (six).
This experiment selects with_startup_window instead, retaining rolling
startup reanchors but requiring the original twelve-interval acquisition.
Reference zero_crosses seed initialized12 to match measured interval count.
No electrical/watchdog/tracking/arm limits changed. No flying recovery run.

Frozen/installed captures/reference/fullseed_750/shell-pwm.elf SHA256:
6D34311DB617319F8362AFB834A2011FC2FBB7B1518782E135821D4FE35F3280.
Release-s/thinLTO/build0/TIM16helperaudit/download/OpenOCDreset0;
own disabled guard3/18 PASS, 57 seed-policy host tests PASS.
After bench tests one comment corrected; source comment not rebuilt.

Exploratory trials, all final-off verified/UART closed/no300 ACK:

- fullseed_750_direct_catch40_drive100.txt: direct100->200,
  sine40->62/forced100/phase60/BEMF70. Forced40ms window times out,
  48commands/42accepts, handler28us/late34us, no measured handoff,
  averagecause0. Final seed state intervals10/cycles5/fault0.
  Retained early trace repeatedly misses step1. This is not wiring proof.
- fullseed_750b_start100_bemf150.txt: same path, sine40->100 and
  requestedinitialBEMF150. Forced40ms timeout48commands/46accepts,
  handler28us/late39us, no handoff, averagecause0.
  Early trace accepts ALL six steps: epoch6step1 accepted4965us,
  then epoch12step1 at9672us. Increased sine drive removes that systematic
  missing-step pattern in this prefix, so do not call it a bad input wire.
  Qualification nevertheless resets on a single spacing>2000half-us ticks:
  epoch5->6 spacing1082us, epoch10->11 spacing1006us, epoch15->16 spacing1024us.
  These near-boundary events prevent the twelve-interval window even though
  adjacent accepted intervals compensate and full cycles are near5ms.
  Final seed intervals8/cycles3/fault0; not a15%BEMF failure (never entered).

Conclusion: full twelve-interval intervention DOES NOT restore practical
startup and is not qualified as a fix. It exposes another distinction between
seed-quality policy and real execution/electrical safety: the1ms individual
accepted-spacing bound resets a still-commanded startup, not demonstrated
tracking loss. Need a dedicated normal-startup estimator that evaluates
measured progression without borrowing flying-acquisition per-edge rejection
rules. Keep actual freshness/current/bus/nFAULT/overrun protections. Do not
spend another cohort trying to roll a lucky twelve-edge window.

Known-good E692/E696 archived ELF lacks the new goal's required average
protection, so it was inspected but NOT flashed as a substitute production
build. No hardware or CPU-headroom conclusion. Goal remains unfinished.
