No blocking defect found in the corrected pure-state implementation.

Removing `Copy`/`Clone` prevents implicit duplication; single ownership still depends on the documented construction contract. Full-width elapsed input makes the half-range rejection meaningful. Strict deadlines, refusal without consumption, cancellation, generation mismatch, and bounded overdue bursts match the stated behavior. At `MAX_AVERAGE_US = 10922`, the final deadline is 32767, safely below the rejection threshold.

The supplied results report 398 passing unit tests and one passing compile-fail doctest; incremental-cache warnings do not invalidate those results.

Limits remain accurately scoped: this code cannot enforce caller serialization, unique generation ownership, or retained elapsed time. The supplied initial gate alone does not establish equivalence to the claimed existing four-rescue schedule. No live-binary correctness claim is established.
