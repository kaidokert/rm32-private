**Role/CCR-generation mismatch is a documented exploratory risk, not a demonstrated remaining blocker to this bounded screen.**

The source deliberately latches roles before the complete CCR generation transfers at a native update. Under the stated latch semantics, UDIS prevents partial CCR publication. The diode-role model exhausts independent channel compare levels: inherited unequal CCRs can alter source pulse width, but cannot request a high on the designated sink or floating phase. The masked ownership check also addresses a pending guard interrupt without subsequently resuming this writer.

Limits matter: the 396-count test establishes the calculated startup bound, not measured runtime CCR history; that bound permits roughly 29.7% transient duty despite a 15% command. The pad output reports self-test PASS, without exposing transition waveforms. Neither establishes analog commutation safety or temperature margin. The incremental-cache warning does not invalidate the 18 passing tests.

I would allow **only the specified single protected screen**, with its 3 A limit, prop removed, guards active, no restart/escalation, and cooldown. This clears the specific mismatch objection; it does not qualify the image or prove the broader switching behavior harmless.
