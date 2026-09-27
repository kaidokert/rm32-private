Raw evidence: `resume_after_refusal` samples the clock after the exclusion block and wake request, subtracts `comp_entry_raw`, adds the existing injection, updates `call_max_us`, and trips through `handler_overrun` when elapsed exceeds 50 µs.

Yes: this addresses the omitted refusal-tail budget through that sample, including COM preemption completed before it. It detects an overrun retrospectively; it does not prevent the wake from executing first.

The correction adds no explicit rearm or authority restoration. However, “without revival” remains conditional on the unseen `revisit_delivery::resume`, TIM16 authority checks, and callers supplying the original COMP entry timestamp.

It still excludes final accounting/exit and entry latency; the 16-bit subtraction also assumes less than one clock wrap. Supplied tests establish threshold semantics, not IRQ integration. The pending real-IRQ handshake test remains necessary evidence.
