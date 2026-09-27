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

Fix1 E492 PASS short regression:60%14.775s,reason2,late0/thin0,guard0,
ceiling600,forced0,coast2167 vs baseline2168eHz,proxy2379 vs2426mA.
COMPmax16 unchanged; COMlate9 vs12us; guardgap108 unchanged. No causal speed/
current claim. Fixture exits1 for predeclared shorter-than30s dwell only.
Fresh MCP alloffPASS,Uartclosed. Keep fix1; one run is not reliability proof.

Before fix2: use stop/blank-dominant comparator resume only on powered paths.
Baseline has no phase4: permit detector only in phase0 and live COM authority;
driven-only acquisition remains permitted with guard clear. Preserve unpowered
comp_exti_arm use by providing a separate powered-arm wrapper for COM callers.
No timer retry state imported. Tests must cover detector precedence, stops,
all blank/commute phases and acquisition; then inspect changed ISR cost.

Fix2 source audit found every comp_exti_arm caller is powered (COM or driven
boundary/begin), so no separate unpowered wrapper was needed. Deferred-driven
resume now also refuses pending after stop. Truth-table/negative cases pass;
352tests/release/clippy PASS. SHA B123ADFA45312299A97BC084821C9AC1A03678C86BB053F2A8742D105B499F0D.
ISR audit4rootsPASS. COMP742->818,COM332->368 listedinstructions, DMA37/guard155
unchanged. New inline checks are after refusal/on-unmask; compiled mask encloses
flag reads/EXTI writes, no new helpers/loops. Existing COMP maximum excludes the
final resume tail, so it is not full WCET. No claim this is a speed optimization.
Single short60screen next, same stops, >120sOFF since E492. Stop on regression.

Fix2 E493 PASS short regression:60%14.775s,reason2,late0/thin0,guard0,
ceiling600,forced0,coast2178 vs baseline2168eHz,proxy2342mA. COMPmax16,
COMlate10us,guardgap105us. Those maxima do not cover the resume tail's full
cost. Fresh MCP alloffPASS,Uartclosed. No observed short-run regression.

Before fix3: inside existing foreground revisit mask, reject a cached sector
that differs from current detector step or a non-idle COM timer phase. Keep
the caller's retry quota; do not substitute another sector. No ISR changes.
Prediction: stale-sector tests fail old admission and pass corrected admission;
four ISR streams unchanged; one short60screen after cooldown.
