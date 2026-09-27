# Loaded baseline: isolated safety backports

2026-09-27 operator requests tag then one fix at a time, checking60% each time.
Baseline source299ba32; release-s/thinLTO/codegen1 advance-ref,deep-filter.
Archived ELF81F85AB5 SHA72581795EFE2E9865E9132E18A70FEAE8F370BD1C60FE0D81A2083CE9A9F247F.
Prop installed, different physical1404/4500KV motor, reverse, enclosed with
restricted airflow. Last operator-confirmed PSU3A. No thermal sensor.

E49050%19.773s and E49160%14.775s completed deadline2,late0,thin0,
no guard/foldback,finaloffPASS. This is a short-run regression anchor, not
renewed sustained qualification or thermal certification. Captures archived here.

Sequence: stop-dominant foreground writes; stop/blank-dominant comparator
resume; fresh-sector foreground revisit; coherent accepted-event telemetry;
host-only audit hardening. No diode mode, timer-owned revisit, carrier changes,
new advance, protection changes or scheduling redesign.

Each candidate: targeted tests, release/clippy, four-root arithmetic audit and
assembly comparison. One45s total60% screen (~14.8s actual target), >=120sOFF
between runs. Require target/ceiling600,deadline2,late0,thin0,no guard/foldback,
finaloff; retain failures, stop sequence on failure. Timing/maxima and matched
speed reported, not attributed causally from a single run. No claims of zero
regression probability from one passing screen. Source/image hash per step.

Before fix1: source confirms foreground moe_on/apply_plan/nonzero compares
can resume after a guard stop. Recheck active/nonlatched atomically around only
those writes; zero initialization remains allowed. No timer/ISR redesign.
Prediction: negative-control test reproduces overwrite; fixed ordering keeps
stop dominant. IRQ streams expected unchanged; bounded foreground mask assessed.

Fix1 staged A0A43CCBE70F1CA5C6510109A21D8BB1B1BF62D6F56F774FC0E73A3AC5870367.
350host tests/release/clippy PASS. Missing ignored replay fixture initially
blocked tests; restored exact e121 capture, no test skipped. Four roots clean,
normalized streams37/742/332/155 identical (TIM16 literal address relocated).
Six foreground masked sites conditional overcount65..393cycles/~1..6.14us;
no calls, at most fixed3phase loop. Not hardwareWCET. Target gate remains60%.
Self-review: mask defers guards/COM during writes; no thresholds altered and
zero initialization remains legal. No claim that this race caused prior faults.
One identical45s60% screen next; no retry if it fails.
