The supplied totals reconcile: 1090 + 605 + 235 = 1930 cycles, or 30.15625 µs at 64 MHz.

Explicit branch recognition fixes the `bics` CFG misclassification. Its remaining charge inflation is conservative. The topological check supports acyclicity after removing the designated persistence edge; the twelve-iteration bound remains a separate premise.

The shown cache stores use bounded `DET_FILTER`, with initialization before activation and refresh in `finish_accept`. History tests support cached/direct equivalence. However, source-string assertions establish placement and occurrence counts, not that every accepted control-flow path reaches the refresh.

No blocking defect is demonstrated in these narrow corrections. The pinned ELF’s complete control flow and shutdown body are reported prior evidence, not independently verified here. The result remains a conditional instruction/fetch over-count, not hardware WCET or power authorization.
