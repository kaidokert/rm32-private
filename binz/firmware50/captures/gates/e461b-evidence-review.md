Static review only; no execution evidence. **Case 7 should produce flags=7**, because `prepare_crossing()` clears TIM16 pending but leaves the software-pended ADC_COMP pending. Its `flags & 7 == 7` assertion correctly permits this.

Recomputed expectations:

- Cases 0–5: `(requests, phase, armed)` = `(0,4,1)`, `(1,4,1)`, `(1,0,0)`, `(0,4,0)`, `(0,4,0)`, `(0,3,0)`. Request counts for 1–2 depend on live admission; `revisit::admit` is absent.
- Case 6: average=1000 yields deadlines **501,1501,2001,2501,3001 µs**. Five pre-ack UIF witnesses plus timestamp windows measure actual peripheral expiry/service, conditional on a passing run.
- Case 8: flags=13 requires `comp_exti_mask()` to clear ADC_COMP NVIC pending; its implementation is absent. The explicitly injected stale dispatch should have UIF=0, phase=0, timer stopped, requests=0.

Cancellation coverage is immediate pending-bit clearance, replacement expiry, and harmless post-stop stale service. Cleanup prevents observing whether the canceled 4000-µs event later resurfaces. Case 8 also clears `active`, so it does **not independently isolate stopped-latch enforcement**.

The corrections improve the probe, but supplied code cannot fully validate flags=13 or admission. Unflashed checks establish no hardware result or WCET bound.
