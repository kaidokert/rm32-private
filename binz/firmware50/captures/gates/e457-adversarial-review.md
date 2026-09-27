**Not established by this excerpt.** A concrete counterexample exists if COMP can preempt COM and accept while phase 4 is being serviced:

1. The phase-4 timer expires. COM enters, acknowledges the update, and passes its `active` check.
2. Before COM loads `phase`, COMP preempts it and accepts a crossing.
3. COMP publishes the payload and sequence, then `prepare_crossing()` clears pending TIM16 state and `start_crossing()` arms the future commutation with `phase = 1`.
4. COMP returns. The already-active COM invocation resumes, loads `phase == 1`, and immediately executes the commutation intended for the new deadline.

Clearing NVIC pending cannot cancel an **active, suspended handler**. The payload can be perfectly coherent while expiry ownership is wrong. Atomic arming does not bind a handler invocation to the expiry that dispatched it.

This is a feasible counterexample under that priority/admission configuration, **not proof that the actual binary permits it**. The supplied code omits the priority assignments and COMP acceptance-entry restrictions needed to establish reachability or exclude it. Comments do not resolve those omissions.

Conversely, within the shown masked crossing-arm transaction, an ordinary guard IRQ cannot interrupt validation and enabling; after a completed stop, the latch prevents another arm, assuming `arm_allowed` implements the stated predicate.

Missing powered latency measurements are separate: they neither prove nor repair expiry ownership.
