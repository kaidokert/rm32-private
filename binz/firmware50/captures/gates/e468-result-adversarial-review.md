**Conclusion: observed handoff failure; causal attribution remains unproved.** E468 records six accepts, seven commutations, then stale-event shutdown. That establishes loss of accepted-event continuity, not whether physical crossings disappeared, persistence rejected valid crossings, or earlier accepts were false. Shortened sampling cadence is plausible; Boolean equivalence and 411 offline tests do not establish temporal equivalence. E465’s successful hold provides context, not a matched causal comparison.

**Archived-control admission: conditional, not established by this packet.** One exact-SHA baseline run could test present baseline operability. However:

- “Same guards” requires verification against the archived binary; the supplied diff is not a complete E465→E468 provenance record.
- The printed `DIODESELFTEST ... PASS` is unconditional on each `p` command after startup checks. It does not demonstrate freshly executed postflight self-tests.
- Historical coast speed implies approximately 21,560 mechanical RPM at six pole pairs. Previous survival, propless operation, a 3 A PSU setting, and 120 seconds OFF do not establish mechanical or thermal safety.
- Verify independent shutdown, physical containment, and enforceable exposure bounds before admission; an exact binary hash alone cannot establish these.

If those conditions are already documented, admit **one** archived control with unchanged aborts. Any failure ends testing; no automatic retries. A pass neither proves regression nor qualifies E468.
