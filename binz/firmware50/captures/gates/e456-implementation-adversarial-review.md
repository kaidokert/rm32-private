**Blocker: admission and reservation occur outside the commit exclusion.** `live` samples comparator line, pending, and level; `schedule.observe()` then reserves a request. The commit rechecks only `schedule.owns(&authority())`.

Concrete interleaving: admission sees the expected level and no pending edge; guard preempts without stopping; comparator level changes; execution resumes with unchanged powered/phase/generation/step. Commit still calls `hw::comp::pend()` using obsolete admission. A newly pending hardware edge likewise does not invalidate authority. This violates the budget’s explicit requirement to serialize live admission, reservation, and actual pend.

The same gap leaves elapsed time stale. A delayed commit can issue a request after the `0x8000` cutoff or arm `decision.delay_us` relative to a later instant, shifting the intended observation deadline. Fresh authority does not validate either condition.

Move fresh clock sampling, comparator admission, `observe`/reservation, and decision application into the same bounded exclusion. If preparation must remain preemptible, revalidate admission and time and recompute the decision before committing.

The supplied tests exercise the pure model; they do not cover this interleaving. The ISR-root audit does not establish temporal correctness. I would block the powered screen until this commit gap is closed.
