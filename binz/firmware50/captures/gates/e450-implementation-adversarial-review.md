Concrete blockers:

- `Board::drv_resume_deferred` remains an unchecked powered writer. A guard trip after its outer deferred/count checks but before its critical section clears ownership and masks COMP; the resumed foreground then unconditionally enables and potentially pends COMP. PRIMASK prevents interleaving inside that section, but does not validate permission.
- `comp_exti_arm` now refuses whenever both detector and driven flags are false, disabling its acquisition use through `Board::comp_arm`. Preserve an explicit acquisition permission or demonstrate that no required caller uses that state.

Proof limits:

- Shown COMP refusal and COM phase-3 branches correctly check permission and write enables under saved/restored PRIMASK; detector ownership takes precedence. COM’s out-of-line `comp_exti_arm` assembly is absent.
- Enable sequences set EXTI IMR bit 18, clear NVIC pending bit 12, then enable IRQ12. EXTI pending survives these resumes; NVIC pending does not. Software-pended events therefore require separate analysis.
- Public-arm edge selection/clearing remains outside the mask; current-owner checks do not validate the caller’s cached sector.
- Tests duplicate the predicate and assume shutdown behavior. The writer-count test searches only `roots.rs`, missing the concrete board bypass. Passing tests/audit establish neither complete ownership coverage nor timing bounds.
