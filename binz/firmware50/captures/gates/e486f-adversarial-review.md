Two concrete accounting gaps remain:

- **UIF is not a post-successor-COM witness.** It can become sticky before that COM. The subsequent 43 µs proves elapsed time, not CCR transfer: `latch::apply` suppresses updates with UDIS. Without an execution bound, subsequent latch transactions can overlap native wraps. Hold can therefore start without demonstrating that the target compares became active. Require an update witnessed after the qualifying COM.
- **Hold acceptance accounting crosses the boundary.** `hold_start` is set before `det_poll`; `consume` then credits the entire returned acceptance batch to `hold_acc`, including pending pre-hold crossings. Snapshot or partition that batch at the boundary.

The masked retiming transaction does exclude guard execution throughout staging/publication. Its acceptable protection delay remains unestablished; instruction identity supplies no WCET bound.

Disabled boot checks support timer behavior, not energized transition safety. Exact-flash verification, prop removal, current limiting, and no retry constrain exploration; they do not resolve those timing/accounting gaps.