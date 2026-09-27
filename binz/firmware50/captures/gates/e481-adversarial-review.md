Verdict: candidate remains unpromoted. E481 demonstrates early closed-loop tracking loss, not failure at 25%: six accepts, ~4 ms closed, CCR 133/1333 (~10%), no target dwell. Neither current nor supply causation is established.

Run order is a serious confound. The same image first sustained 15%; E481’s cumulative acceptance sequence and boot-cumulative recheck counters show retained bookkeeping, but do not prove harmful retained control state. Passing preflight establishes bridge-off conditions, not estimator, filter, timer, or pending-recheck reset correctness. Nearly identical driven seeds and qualification counts localize the divergence after handoff without explaining it.

Carrier/persistence interaction remains plausible: a ~2.08 µs ON interval could interact with a microsecond-scale persistence check. But neither the actual filter duration nor the relevant comparator-valid window is established here. E480 also accumulated many “unstable” rejects while running successfully; that counter alone is not diagnostic.

Smallest next step: a focused source audit tracing ordinary-run teardown/reinitialization and the first closed-loop sectors: retained state, pending events, actual carrier/CCR, filter depth and execution timing, and recheck ownership. The supplied policy/helpers cannot establish those paths.

Slower carrier entry followed by 48 kHz merits that audit as a bounded control experiment, not a proven fix. Predeclare transition, startup endpoint, and fixed comparison budget; preserve guards and stop afterward—no pass-fishing.
