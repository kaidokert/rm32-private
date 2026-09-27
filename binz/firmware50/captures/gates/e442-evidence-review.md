Raw baseline: 25% propless reached 3.371 s target hold, zero forced commutations, and one late arm at CI=40 µs, spent=10 µs. Disabled outputs were observed before/after. This establishes prior exposure, not candidate safety or timing improvement.

Publication is conditionally correct: one non-reentrant ADC_COMP writer permits atomic relaxed load/wrapping-add/store without lost increments; the release compiler fence orders payload before publication. Foreground’s bracketed reads work because the ISR completes before foreground resumes. A COM reader preempting publication may see an old sequence with newer payload; correctness depends on its actual use, which the reference list does not establish.

Concrete evidence blocker: `System.Object[]` provides no assembly comparison. Before screening, show both changed publication sites, payload→sequence→arm ordering, preserved arm/stop masks, and the persistence-loop branch/context. Tie that evidence to the exact frozen candidate ELF. Audit/test passes do not establish those properties.

With that gap closed and COM use justified, one proposed 25%, 28 s propless screen after fresh flash/disabled boot and ≥120 s OFF, with no retry, is proportionate. It demonstrates feasibility only; no unrelated ladder is required.
