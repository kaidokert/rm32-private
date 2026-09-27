Exact historical preservation is **unverified**: `RAW HISTORICAL SOURCE` contains only `System.Object[]`, not the historical method. The commit identifier and “retained verbatim” comment cannot establish exact preservation.

The supplied oracle **is independent of the candidate adapter**: it implements the gate, read loop, and state updates directly without calling either current entry point. Its historical provenance remains unproven.

Regression assertions provide substantial digital coverage:

- Exhaustive streams through depth 12 independently assert acceptance and exact read counts, including both polarities, zero depth, and the default gate boundary.
- Stateful sequences compare complete outcomes, estimator state, `prev_zc`, counters, and read counts, exercising depth 255, extreme inputs, wrapping counters, and four timing policies.
- State equivalence is differential against the supplied oracle; shared arithmetic helpers leave common helper regressions undetected.

The exhaustive test hardcodes a gate of 40 through `ZeroCross`; its expectations require the default blanking configuration and fail under `wide-blank`.

The reported 382 passing tests support the executed configuration, whose full feature list is truncated. They do not authenticate the historical body or establish physical timing equivalence.
