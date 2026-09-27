The raw result is **10 intermediate-low observations and two double-high observations in a settled-state model**. It establishes neither physical pulse widths nor harmful current nor the cause of sag. Short-pulse suppression, dead time and write timing remain unmodeled.

**Yes: a scoped offline candidate is justified**, with these unresolved risks:

- **Atomicity is assumed by the second test.** `active = preload` implements the desired conclusion. It never exercises CCPC, COMG, partial staging or an actual interrupt. Likewise, same-leg exclusion follows algebraically from `reference` versus `!reference`; it cannot validate hardware shoot-through prevention.
- **Role atomicity is not compare atomicity.** CCR transfers follow their own preload/update rules. An overflow between CCR writes, or a role change affecting OCxPE, could expose mixed compare generations. Equal requested compares do not prove equal active compares during a duty change.
- **Stop ownership extends beyond MOE.** A guard interrupt can zero CCRs, then resumed COM code can overwrite those zeros. MOE remaining clear prevents immediate drive, but the promised post-stop state and later restart assumptions can fail. Establish abort/invalidation behavior before accepting the transaction.
- **CCPC temporarily changes other helpers’ semantics.** Audit every reachable CCER/CCMR writer, including safing and panic paths. A disable intended to be immediate may become staged.

Discriminating offline tests: inject guard safing after every register operation; enumerate native updates between writes with differing old/new duties; model active/shadow registers separately; verify final CCRs, CCPC and restart state. Audit generated stores and added COM cost. Hardware behavior and causal attribution remain unproven.
