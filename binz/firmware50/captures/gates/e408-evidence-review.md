The reference chronology supports the proposed one-event delay: acceptance arms COM using the **previously stored waitTime**; the ensuing COM callback commutates, blends the latest ZC intervals, and computes the wait for the next acceptance. Firmware50 currently blends first and schedules from that fresh estimate.

At fixed advance16, PreviousEstimate reproduces that dependency **provided** each accepted event has its corresponding COM update before the next acceptance, estimator inputs/units match, and no intervening path changes the estimate or wait. It is not full AM32 equivalence: update timing, blanking, clamping, timer semantics, and startup initialization remain separate differences. Snapshotting before `offer` avoids introducing an uninitialized cached wait, but does not establish reference-equivalent seed initialization.

Arithmetic to verify:

- Reference wait is `(ci >> 1) - ((ci * 16) >> 6)`, not generally `ci / 4` because of truncation.
- Confirm firmware50’s `wait_time` accepts an **advance level**; minz’s shown call instead passes the computed **advance amount**.
- Preserve existing floor, timer offset, narrowing, and atomic late-stop checks. Reference arms `waitTime + 1`.

Required tests: a multi-event sequence proving previous-versus-fresh selection; identical rejection state/counters; first seeded acceptance; odd estimates and floor boundaries; production width/overflow behavior; both plain/logged paths; unchanged publication/blanking/watch behavior; capture policy identification. Build and helper audits remain necessary before flashing.

The screen establishes a protected sag stop with **zero target hold**, not 60% qualification. The proposed sequential 50/conditional60 batch is exploratory; historical BF results do not provide a matched causal A/B.
