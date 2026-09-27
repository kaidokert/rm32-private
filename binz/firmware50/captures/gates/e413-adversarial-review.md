**Conditionally admit the disabled boot-pad test; do not admit motor operation from this evidence.** The witness deliberately asserts MOE and gate-input pads, so admission depends on DRV ENABLE remaining physically low throughout.

- **Stop ownership:** The masked `arm_allowed(stopped, active)` check covers a completed `guard_trip`, because it clears COM active. A guard arriving during the transaction is deferred until after COMG: a new sector can therefore become active before shutdown. This is bounded stop latency, not “the guard always wins before transfer.” The extra ~50 instructions are not a measured worst-case bound.

- **CCR coherence has an entry invariant:** Enabling all OCxPE bits before writing CCRs prevents partial *new* generations. It does not establish that existing active CCRs are equal. COMG changes roles before the next native update; a newly selected source uses its previous active compare until then. Verify equality at every entry path, especially startup and recovery. Forced-mode, zero-compare pad testing cannot establish this.

- **Witness coverage is narrow:** Three delayed pad samples demonstrate settled staging/transfer behavior for one transition. They cannot exclude short glitches, establish dead-time behavior, or validate UDIS/CCR transfer under overflow. The host models assume the hardware semantics being challenged.

- **Configuration ownership:** `latch_check::run` changes CR2 and role state without restoring the original configuration. Explicitly establish its required exit state and that subsequent initialization supplies it. ENABLE’s initial check also needs exclusive ownership throughout the witness.

- **Evidence gates remain open:** The supplied ratchet failed and the final 372-test rerun is pending. A separately named bucket preserves history but does not justify the latency increase.

Also bound/reject excessive duty caps and the `period=65536`, 100%-duty CCR truncation.
