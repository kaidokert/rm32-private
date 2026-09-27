**No reachable phase-1 COM change to COMP’s pre-mask sector snapshot is shown**, given your stated `comparator_resume_allowed` restriction to phases 0/4. That predicate’s implementation is absent from the packet, so this part is a premise.

The reachability argument is:

- **Startup:** `drv_end()` masks and clears COMP before `det_install()` activates the detector. Installation does not reopen the line. `com_handover()` installs the initial phase-1 timer under PRIMASK while COMP remains masked. Its phase-1 service advances the sector before reopening COMP, either immediately or through blanking.
- **Normal acceptance:** COMP masks its line at entry. Before this invocation arms phase 1, an optional phase-4 service can preempt, but cannot advance `step`; once it observes the masked line, it parks. Refusal can wake that observation, not create phase-1 commutation.
- **Crossing arm:** persistence, acceptance publication, and crossing-arm preparation/start execute inside `Masked::run`. Phase-1 COM cannot intervene there. Acceptance does not itself reopen COMP.

**COM can advance the sector after the arm**, when `Masked::run` restores PRIMASK—even before `accepted_arm` bookkeeping begins. It may then reopen COMP while the current COMP handler is still finishing. That handler cannot preempt itself; its saved `step` describes the accepted crossing, not necessarily the now-current sector. This is post-arm overlap, not corruption of the pre-mask snapshot.

Thus the proposed pre-mask phase-1 counterexample lacks a reachable timer/line state under these premises. Pending-guard delay remains a separate unresolved question.
