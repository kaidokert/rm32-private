Evidence: `guard_arm` resets the watch before activation; `Seam::lock` can refuse, and its result is discarded. Reporting follows `safe_off`, coast capture, baseline measurement, and driver disable. The snapshot masks interrupts; printing occurs afterward. Host tests report 385 passes.

**Concrete blockers:** No unconditional blocker demonstrated. However, reset failure is silently accepted: handler-mode invocation or an existing foreground borrow leaves prior-run evidence intact while arming continues. If either is reachable, this correction fails its stated purpose. Require successful reset or establish those call-site invariants.

**Proof limitations:**

- The source-string test proves textual ordering, not reset execution or exclusion of concurrent rearming. `guard_arm` also depends on the previous guard being inactive/disarmed during initialization.
- Snapshot masking provides consistency against maskable ISR writers on this core, not fault-time attribution. `reason`, `ticks`, and `gap_max` are loaded separately afterward.
- `from_event` identifies the software decision path, not a physical edge. Tightening immediately before `event` can explain staleness.
- Added fault-path stores precede shutdown. Audit success and reported instruction identity do not establish their latency or explain how those stores appear in the audited image.
