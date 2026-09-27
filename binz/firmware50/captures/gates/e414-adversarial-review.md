Yes: the role/compare mismatch remains a concrete safety gap before the proposed powered screen.

- **Safety:** COMG installs new roles before the next native update installs new compares. The new source therefore uses an inherited active compare. Bounding each *plan* to 80% does not bound that inherited value against the current ARR, especially across the direct 10→48 kHz handover. Establish that bound explicitly. Forced-mode pad results `24→24→9→0` verify role staging, but cannot demonstrate PWM pulse widths or transition glitches.
- **Concurrency:** The ownership check inside PRIMASK addresses resumed writes after a serviced stop. It does not establish acceptable latency for a guard becoming pending during the transaction. The model assumes deferred servicing and omits role outputs, timer phase, and break behavior. Instruction counts and arithmetic audits do not close those gaps.
- **Next discriminating test:** With ENABLE held low, scope all six timer pads through actual PWM handover and latch transactions, including unequal inherited CCRs and updates near COMG. Inject a pending guard and measure request-to-output-disable latency. Check for excessive pulses, conflicting gate commands, and reactivation after stop.

After those checks, one protected 50% screen is defensible. A clean 50% run alone does not justify the conditional 60% escalation.
