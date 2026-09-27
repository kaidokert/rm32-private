**Do not flash expecting the `u` suite to pass unchanged.**

- **Case 8 contradicts its setup.** `guard_trip()` calls `com_stop()`, which explicitly unpends TIM16. The refused `com_arm_crossing()` never repends it. Consequently `cleared == true`, making `flags == 9` impossible: bit 2 must also be set. No intentional TIM16 event remains to produce the required single callback. To test stale dispatch after stop, explicitly inject that dispatch after stopping.

- **The timeout is not independently bounded.** Both waits depend entirely on `hw::clock::raw()` advancing. A stopped clock hangs forever, bypassing cleanup. TIM16’s `PSC=63` assumes a 64 MHz timer input; the supplied code does not establish that clock setup. A shared clock-rate error can also pass relative timing checks.

- **Case 6 can falsely certify scheduling.** Five UIF-backed callbacks and five observations establish counts, but their recorded times are never asserted. Incorrectly compressed slot spacing can pass.

- **Off-only evidence is narrower than “throughout.”** Register snapshots cannot exclude transient activation. `prepare()` checks inactivity before entering exclusion, then installs synthetic authority without rechecking. Safety therefore depends on the binary having no asynchronous owner-start path; the supplied excerpt does not establish that invariant.
