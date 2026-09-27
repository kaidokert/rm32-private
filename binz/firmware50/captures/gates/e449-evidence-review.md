The strongest bounded candidate is **caching persistence depth when the estimate changes, computing the next depth after arming**. Savings remain unmeasured.

Closed-loop acceptance follows entry mask/ack and timestamp (`0x08000610–0626`), rate handling, then `det_decide_plain`: estimator presence/count checks (`07e8–0818`), sector clamp and half-cycle gate (`0822–0846`), depth calculation (`0848–08ce`), persistence (`08de–08f4`), blend/clamp/publication (`08f6–094e`), fixed16 wait (`0950–0958`), atomic eligibility/preparation (`095a–0992`), elapsed sample (`0994`), remainder validation and timer start (`09a0–09c4 → 0a22–0a2e`). Expiry instead calls shutdown at `0a1a`.

`bemf::offer_timed` recomputes `policy.level(self.average_interval)` on every gate-passing offer. The emitted conversion, thresholds, reciprocal multiply and floor occupy `0848–08ce`, with mutually exclusive branches. At average 54, execution takes the interior mapping path and selects five reads. Refusals leave the estimate unchanged, making that calculation reusable.

Cache depth alongside detector state; initialize on construction/reseeding and refresh from the blended estimate after `arm_marked`, before another COMP invocation. Keep both decision twins coherent. This removes scheduling arithmetic from entry-to-arm while retaining five volatile reads and their existing loop spacing. Absolute first-read timing moves earlier; if “aperture” includes that offset, this proposal requires reconsideration.

Proposed equivalence tests: differential depth across thresholds and saturation; decision/state replay with dissent at every read; cache coherence across refusal, acceptance, reseed and refused/expired arms; emitted read-spacing and publication/atomic-arm ordering checks.

Guard and margin accounting execute after arming. Their size cannot establish pre-arm savings. Capture `41 → wait 10`, `spent 10` supports expiry arithmetic, not a physical explanation.
