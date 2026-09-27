Raw disassembly supports the intended nesting: outer `CPSID` at `0x080008b4`, persistence inside it, timer enable at `0x08000ad2`, inner restore at `0x08000ad6`, outer restore at `0x08000ae4`; margin bookkeeping follows. The inner restore preserves masking. No demonstrated mask-restoration failure.

Two material obligations remain before motor testing:

- **Pre-mask snapshot validity:** `start/count/step/depth` are captured before masking. A COM between those reads and `0x080008b4` could leave persistence evaluating the previous sector’s polarity and interval. Show why that interleaving is unreachable, rejected, or harmless; otherwise revalidate the sector/epoch inside the window. Masking persistence through arm alone does not establish this.
- **Guard blocking:** a guard becoming pending during persistence cannot execute before timer enable. Establish whether arming while that guard is pending is permissible, and bound the resulting delay through actual shutdown. A pending interrupt is not equivalent to an already-published inhibit flag.

The concrete next gate is an offline, path-complete blocking proof from outer masking to restoration, including maximum reachable depth, acceptance/refusal branches, nested arm, and `stop_expired_arm` → `guard_trip`. Include target flash/bus timing and timer-expiry behavior while masked; compare against explicit guard and commutation budgets. Follow with drive-disabled pending-guard/pending-COM tests.

Passing tests and ISR audits do not supply these bounds. No flashing or powered test is justified yet.
