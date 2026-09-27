No removal-specific counterexample, provided `AVERAGE_MIN_US == 32` and cancellation sets `slot = 5`.

For valid averages, `used <= slot` holds initially. At each due observation, slot advancement strictly increases `slot`; `used` increases by at most one. Request deadlines increase with their index, so `Budget`’s deadline is never later than the observation deadline. Thus reservation equals `live_admitted` after the outer checks. Invalid averages terminate before arithmetic; the largest valid deadline is 32767.

Shared limitations remain:

- **Callbacks:** arbitrarily many calls at elapsed 0 return the same delay. Five bounds consumed observation slots, not callback invocations.
- **Clock wrap:** origin 0, first observation after `2^32 + 41` microseconds, `now = 41`, average 80: both implementations pend stale work.
- **Ownership:** an unobserved stop/restart restoring identical authority, or generation reuse after wrap, can make stale work appear owned. Serialization and obsolete-IRQ cancellation remain external requirements.
- **Skipped deadlines:** elapsed 300 with average 80 produces one request and terminates in both implementations.

The supplied hardware trace does not establish those external guarantees.
