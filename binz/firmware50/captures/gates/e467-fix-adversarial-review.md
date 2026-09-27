Yes—the frozen `old_offer` resolves the shared-implementation oracle problem for the comparisons shown. It independently preserves the prechange gate, persistence decision, state updates, and estimate selection. Sharing unchanged arithmetic helpers does not undermine independence for this refactor.

The exhaustive cases verify acceptance/rejection, exact read counts, outcomes, and compared state across both polarities, every depth, and every first-dissent position. Histories extend this to evolving state, gate boundaries, extreme seeds, and varying advances.

Two coverage limits remain:

- These comparisons exercise `offer_matching_timed`, not the `offer_timed` polarity adapter.
- `DepthSnapshot` does not verify when, how often, or with which interval `policy.level` is called.

Neither invalidates the stated oracle repair. The 411-pass result supports the tested scope; histories are sampled, not exhaustive.
