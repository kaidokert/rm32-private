The packet reports 379 passing tests, four clean ISR-root audits, and compile failure when the controller schedule is not 16. Both supplied advance writers use `P::A::level`; `Production` selects the policy whose two levels are asserted equal to 16. The constant ISR deliberately ignores runtime advance stores.

Disassembly shows:

- Fixed16 wait retains `(ci >> 1) - (ci >> 2)` rounding.
- Exhausted waits branch to `stop_expired_arm`.
- `spent` is sampled at `0x0800098e`, before timer enable at `0x08000a1e`.
- OrderRing writes occur after arming.
- The reported stack frame grows by eight bytes; 813→787 includes literals.

These support arithmetic equivalence for the exercised inputs, not identical physical behavior. Earlier live comparator reads can change acceptance; recording after arming can still affect interrupt service and subsequent crossings. The smaller disassembly establishes neither latency improvement nor WCET. `spent` excludes interrupt-entry latency and the remaining timer-start path. The historical “cannot reach zero” argument also fails at `wait = spent = 10`.

No concrete blocker to the proposed **single 15%, 28-second propless screen** is established. Support is limited to the identified candidate, successful disabled boot checks, 12 V/3 A, unchanged guards, and 120 seconds off. Verify flashed identity and observe actual stop/current/timing results. Prior 25% LateArm failure remains unresolved; success would justify neither escalation nor an uninstrumented-image claim.
