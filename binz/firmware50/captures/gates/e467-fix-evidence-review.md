Yes—this resolves the shared-implementation oracle concern for the tested logical equivalence, read counts, and histories.

`old_offer` independently reproduces the supplied prechange gate, persistence decision, state updates, and scheduling selection without calling either current API. Sharing unchanged arithmetic helpers is appropriate for this scoped review.

The exhaustive cases cover every depth, both polarities, every first-dissent position, and full acceptance for the three instantiated timing policies. History cases compare outcomes, read counts, state, counters, and `prev_zc` across gate boundaries and extreme seeds, including zero reads on gated rejection.

Limits: these comparisons exercise the matching API directly, not the compatibility wrapper; `DepthSnapshot` does not test dynamic policy evaluation. The reported 411 passes support the evidence but do not independently establish test inclusion. The host mask test establishes algebra only.

Verdict: the requested oracle-independence issue is resolved within this scope.
