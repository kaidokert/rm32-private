- **High — case 12 can falsely certify shutdown.** `handshake::run()` calls `finish()` before checking `S.com().stopped`; `finish()` unconditionally invokes `guard_trip(HostAbort)`, which sets that latch. The COMP handler also calls `finish(1)` after recording reason 14. Replacing the overrun shutdown with only `reason.store(14)` could still pass. Capture stopped/active/CEN immediately after `resume_after_refusal()`, before either cleanup.

- **Medium — cancellation coverage is weaker than claimed.** Cases 10/11 wait until the original expiry has dispatched and been acknowledged before acceptance/stop. Neither creates an outstanding refusal wake to cancel. Case 11 then deliberately pends a fresh interrupt. Add scenarios with a wake pending before acceptance/stop; verify cancellation without cleanup supplying it.

- **Recomputed:** average 80 gives deadlines **41, 121, 161, 201, 241 µs**. Cases 9/12 begin at age 60 plus setup/parking time; one observation is plausible, but case 9’s rearm requires age **≤240 µs**.

The shown probe paths do not enable the bridge. Priority configuration remains a prerequisite: without `com-top`, COMP’s parking wait cannot receive TIM16 preemption.
