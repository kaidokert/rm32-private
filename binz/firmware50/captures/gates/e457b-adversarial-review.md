**No feasible publication/expiry ownership violation is established by the supplied paths.** With `com-top`, COM is actually `0x40`, COMP `0x80`, and guard `0x00`. COM can interrupt COMP; COMP cannot interrupt COM.

The strongest attempted counterexample is COM phase 4 interrupting acceptance between payload stores and sequence publication:

- COM still sees the old generation, so ownership can pass. However, phase 4 uses its saved schedule; the newly read acceptance origin is unused. COMP has already disabled IMR, so live admission fails. COM may schedule another phase-4 expiry.
- COMP resumes, publishes the new sequence, then enters masked crossing preparation, which stops the old timer, clears its flags and clears TIM16 pending before starting phase 1.
- If that old expiry interrupts **after publication but before preparation**, generation mismatch retires the schedule without changing phase or arming a timer.

Once crossing preparation starts, PRIMASK excludes COM and guard through validation and timer start. A resulting phase-1 dispatch therefore sees completed publication. COMP cannot change that payload during COM’s reads. Guard shutdown removes authority and prevents the suspended acceptance from rearming.

The sequence/fence is **not independently a coherent-snapshot protocol**; correctness depends on these scheduling and admission constraints. Foreground readers and handover are not fully supplied, so this verdict does not establish their ownership correctness or prove the sole-writer claim globally.
