The shown publication/expiry ownership is sound, conditional on `arm_marked` reaching the displayed crossing-arm path. Its body is absent, so that connection is not independently established.

With `com-top`, COM can interrupt COMP; COMP cannot interrupt COM or re-enter itself.

- **Before the sequence store:** phase-4 COM sees the old generation and may observe, pend COMP, or rearm phase 4. Its timing payload comes from the saved `Schedule`. Although `clock_and_origin()` can read newly written `accept_raw`, phase 4 does not use that origin. COM finishes before interrupted COMP resumes. Accepted preparation then disables/stops TIM16, clears its flags and pending interrupt, and installs the crossing timer—or trips the stop.
- **After the sequence store, before preparation:** the old schedule fails generation ownership. `after_phase` retires it before its phase store or timer arm. Acknowledging the dispatched interrupt does not overwrite the forthcoming crossing arm.
- **During preparation/start:** interrupt exclusion prevents COM or guard interleaving.
- **After start:** phase 1 can preempt COMP, but all acceptance payload stores already completed. Suspended COMP cannot publish another acceptance while COM reads them.
- **Stop/expiry:** shutdown clears COM activity and latches `stopped`; subsequent arm validation refuses. Recheck’s masked authority check rejects stopped work.

The compiler fence alone does not establish ownership; these execution exclusions do. Whole-program sole-writer ownership remains unproved by the excerpts.
