The race is real. `Ctx::pass_inner` caches `com_step()`; later, `Locked::revisit` checks `com_idle()` outside Board’s critical section. COMP can accept and TIM16 can commutate between those observations and entry into `Board::revisit`. Admission then combines the new sector’s timestamp and live comparator with the cached old step’s polarity. `det.active` does not establish COM idleness.

Rechecking **current detector step equals the supplied step** and **COM phase is zero**, inside the same interrupt mask as admission and `pend()`, closes that specific interleaving. A mismatch must return false before pending, leaving retry accounting untouched. Checking only COM idle is insufficient: a completed commutation can already be idle in the new sector.

Two limits remain:

- Step equality is not sector identity after a full six-step cycle; an epoch/count check is needed if that case must also be excluded.
- Interrupt masking does not freeze comparator hardware; physical edges can arrive during sampling.

ISR persistence uses the actual detector step, so this source establishes inconsistent retry admission, not necessarily false acceptance or physical causation.
