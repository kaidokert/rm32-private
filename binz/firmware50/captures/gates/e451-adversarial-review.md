Raw: cache semantics are sound if installation initializes depth before publishing `active`, and every acceptance refreshes it before ISR exit. Refusal/rebase leave the average unchanged; COM/guard neither consume nor invalidate depth. Cache the **published blended average**, even with `PreviousEstimate`.

Concrete gaps:

- Default 12 is not universally “fail-conservative”: extra reads consume timing margin and can reject genuine crossings. Require initialization before use.
- Post-arm refresh still delays equal-priority COM dispatch. Checking persistence-loop spacing alone misses this cost; check arm-to-COM latency and total handler time.
- Refresh must cover both plain/logged paths, including accepted decisions whose arm returns `None` or trips LateArm. Keep installation under the existing inactive-detector ownership requirement.

Required tests: cached versus direct policy across multiple accepts, both refusals, rebase, clamps, reinstall with a different seed, and Fresh/Previous scheduling; exact depths around mapping/shallow boundaries under all floor-feature combinations. Compare outcomes, estimator state, and comparator read sequences for both paths. Explicitly verify the initial depth uses `zc.average_interval()`, not `seed_us`.
