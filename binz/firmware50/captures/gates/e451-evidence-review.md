Raw code supports caching: depth uses the pre-acceptance average; only acceptance changes that average. Refusal and rebase preserve it. Refreshing after arm and `guard_event`, before COMP returns, supplies the next decision’s correct depth. COM/guard need no cache access.

Concrete gap: `det_install` does not itself enforce detector inactivity; its initialization precedes the guarded activation. Initialize depth before activation, and require installation with COMP excluded or the detector inactive. Default 12 is not a substitute for exact initialization.

Required tests:

- Cached versus direct decisions, comparator read sequences, state and counters across accepts, both refusals and rebase.
- First acceptance after install/reinstall with different seeds.
- Accepted averages crossing every depth boundary, including clamp hits, µs saturation, shallow override and both floor features.
- Plain/logged parity; refresh despite arm refusal or guard stop.
- Refusal-history assertions: the supplied test checks only average.

Check emitted persistence-loop spacing and pre-arm cost; logical equivalence alone does not establish timing equivalence.
