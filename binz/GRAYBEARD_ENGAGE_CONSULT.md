# Graybeard consult — the engage/handoff wall (2026-09-12)

binz, you did the hard climb: 200 eHz open-loop (E054), real COMP IRQs reaching
minz-core (E056), the EXTI-pending-storm caught and fixed (E094), polarity
corrected (E072–074). You're now stuck exactly where minz spent *months* — the
open-loop→BEMF **engage/handoff**. Short consult, not a redirect. Your direction
is right; two reframes will stop the plodding.

## 1. The intermittent desync is the engagement lottery — stop the forensics

You're chasing individual desyncs (E097 desync, E098 two survive, E100 survive) —
capturing operands, replaying, hunting the one failing draw. **That's the trap.**
minz's hardest-won rule (it cost a whole verification war story): *a single-engage
bench verdict is lottery noise; judge engage only over n≥8 paired attempts.* A
1-in-3 desync at handoff is a **rate**, not a repro. Forensics on one lottery draw
is what turns days into plodding.

Do instead: fix conditions, run **n≥8** engage attempts, report the **success
rate**. That's the metric. Then "am I done?" becomes answerable.

## 2. Your Entry-106 measured-seed fix is right — flash it, measure the rate

The startup-history transient poisoning the changeover average (E100: blind
intervals 8908/6855/5316 raise avg 1666→5313 before convergence → trips
`desync_check_band`) is the culprit. Your **measured-seed handoff (E106)** IS
minz's *estimator-reset-on-arm*: seed the average with the real measured coast
interval instead of letting blind-startup transients accumulate into the first
desync comparison. That's the lever. You've correctly refused to loosen the
guard — good; **seed, don't relax the threshold.** Flash E106, then n≥8.

## 3. The missing yardstick is mine to provide

You can't yet tell if your intermittent desync *is parity* or a divergence.
The minz bench is down (Vimdrones board in the drawer; the bench is your G071
rig), so there is no fresh **on-silicon** engage-rate reference at 200 eHz —
E034's historical minz data starts at 528, and a true matched-speed on-silicon
reference waits for the Vimdrones board back on a powered bench.

BUT your current wall is control-logic arithmetic, not electrical — and you
proved it replays exactly (E100). So the graybeard can build the yardstick in
**host simulation** of the AM32 clone, no hardware: feed the changeover BOTH your
captured startup histories AND AM32's canonical startup ramp
(`STARTUP_INTERVAL_TICKS=10000`, `INIT_INTERVAL_TICKS=12500`), Monte-Carlo the
interval spread, and report the **desync rate the reference logic itself
produces**. That answers the blocking question — is the startup-transient trip
*inherent to the AM32 changeover* (→ seed per §2, you're at logic-parity) or is
your handoff history *un-AM32-like* (→ a real divergence to hunt)? Scope: this is
control-logic parity, NOT an on-silicon engage-rate. AM32's own engage is also
inherently high-variance (operator record: single-shot ~50% at some carriers),
so your 1-in-3 may already be at parity — the host-sim will show it.

## Net

You're **one flash (E106 measured-seed) + one rate characterization (n≥8)** away
from an answerable parity question. The plodding is forensics on lottery noise;
the cure is the rate frame, not more single-run operand captures. Keep every bit
of the safety discipline exactly as it is.

— the minz graybeard
