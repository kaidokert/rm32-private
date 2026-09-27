Yes, narrowly: the second check closes the newly added refusal-tail measurement gap through its clock sample, including intervening COM preemption, while preserving injection and the existing >50 µs cutoff.

“No revival” remains conditional: `resume` must reject revoked authority, and `guard_trip` must invalidate/disarm any work already resumed or armed. Those implementations aren’t shown. Pending TIM16 before checking the budget allows COM to execute before an overrun is detected; this is retrospective detection, not prevention.

Remaining limits: final accounting/exit is excluded, and the 16-bit subtraction aliases durations ≥65,536 µs. The supplied tests verify the threshold, not this integration. The pending real-IRQ test should cover over-budget wake preemption and verify that subsequent TIM16 delivery cannot rearm after the trip.
