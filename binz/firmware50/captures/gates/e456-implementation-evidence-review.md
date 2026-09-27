Two concrete blockers in the supplied implementation:

- **Live admission is stale at commit.** `line_live()`, `pending()`, and `level()` are sampled before `schedule.observe()` and outside the commit exclusion. The commit rechecks only `Authority`. A comparator transition can change the level or set pending without changing generation, step, or phase; the code then issues `pend()` using obsolete admission. This directly violates `Budget::reserve`’s stated serialization contract. Revalidate admission and perform reservation alongside the actual pend under exclusion. Hardware can still change asynchronously; preserve the comparator handler’s final validation.

- **The claimed offline-only report is not enforced.** The binary calls `recheck::report(&mut board)` **before** `controller.command(&mut board, b)`. Thus any stop performed by the `p` command happens after the report has already called the sink. `report()` itself checks neither stopped nor active state. Process the stop/offline transition first and enforce the offline condition before emitting the report.

One additional clock limitation needs resolution: `clock_and_origin()` reconstructs acceptance age from a `u16` difference. An acceptance older than one full wrap aliases to a recent origin, defeating the model’s extended-age rejection. Store an extended acceptance timestamp, or establish the enforced bound that makes this reconstruction valid.

The supplied host tests and ISR audit do not cover these integration paths.
