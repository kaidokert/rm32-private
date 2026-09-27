No concrete correctness defect is demonstrated in the supplied implementation.

- `PreviousEstimate` snapshots the estimate inside the exclusive offer, before blending. Both decision paths forward the policy type; the default wrapper retains `FreshEstimate`. Acceptance bookkeeping and new-estimate publication remain shared.
- The change also selects the estimate for `Outcome::Accepted.advance`, not just `wait`. The shown plain path ignores that field; downstream consumers should preserve this intended distinction.
- Tests exercise seed use, subsequent preblend scheduling, rounding, refusals, saturation/clamping, and frozen-input replay. The reported 369-test pass supports host behavior; replay does not establish changed motor behavior.
- The emitted excerpt shows new-estimate publication before arm preparation, then masked stop/active validation, elapsed subtraction, expiry shutdown, and timer enable. However, the earlier stores establishing `[sp,#4]` and `[sp,#8]` are absent, so this excerpt alone cannot fully prove the wait’s preblend operand provenance.
- `ci_min` constrains the published estimate, not necessarily the selected seed/preblend wait. The implementation and supplied claim correctly distinguish these.
- Audit and ratchet reports support their stated arithmetic/hazard checks; default-root identity supports unchanged default ISR instructions. They do not establish timing equivalence or whole-image identity.

Evidence gaps remain for the exact EFFD1B1C archive/build linkage, fixed16/48k configuration, complete guard configuration, and enforcement of 120s OFF/45s maximum and stop-on-failure screening: those supporting artifacts are not supplied. No powered-screen safety or qualification conclusion follows from these excerpts alone.
