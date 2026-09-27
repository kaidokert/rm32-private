E453 stopped on stale tracking: age 243 µs exceeded 240 µs. The 221 µs maximum foreground gap does not locate that gap at termination or prove a missed retry caused the stop. Hold never began; revisit attribution is further limited by 4,323 coalesced accepts.

The code proves recovery depends on foreground service: `pass()` runs before mailbox consumption and revisit, so an error there bypasses recovery. COM phases 2/3 only reopen the comparator path; they do not request a level retry.

The claimed per-sector quota is not generation-bound. Foreground permits one initial admission plus four rescues, assuming the supplied maximum. Rescues become eligible strictly after 1.5, 2.0, 2.5 and 3.0 cached intervals. Acceptance consumption resets quota; commutation only clears inflight attribution. Board admission always supplies `already_retried=false`; six-step equality cannot distinguish generations. Existing tests establish predicates and textual ordering, not scheduling or quota integrity.

A viable minimal design uses an explicit COM-owned recheck state:

- Identify each sector by generation; charge at most five software pends against that generation.
- Schedule the first check strictly past the live half-interval gate; retain rescue thresholds and revalidate eligibility at service.
- Route pends through ordinary COMP persistence/acceptance.
- Atomically cancel rechecks on acceptance, commutation, or stop; accepted COM scheduling must supersede rechecks.
- Preserve guard deadlines and count only actual commutations.

Test stale-generation rejection, timer/acceptance collisions, failed admission, quota exhaustion, and stop/arm races before coding the integration.
