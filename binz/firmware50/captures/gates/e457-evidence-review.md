**Publication/expiry ownership is not established by this source.** The phase‑1 claim has a concrete contrary interleaving if TIM16 can preempt COMP:

1. An earlier acceptance A has armed phase 1. Its timer expires while COMP processes acceptance B.
2. B stores `sector_start_raw`, `accept_raw`, and `accept_avg`, then is interrupted **before** storing `accept_blank` and publishing `accept_seq`.
3. TIM16 acknowledges A’s expiry and reads phase 1. It advances the step and applies the commutation plan.
4. It reads **B’s average, A’s blanking, and B’s raw timestamp**. Its optional ordinal remains A’s sequence.
5. COMP resumes, stores B’s blanking, publishes B’s sequence, and prepares/starts B’s timer.

`prepare_crossing()` cannot cancel the already executed commutation or repair its mixed payload. The release compiler fence supplies neither exclusion around those reads nor a snapshot validation protocol.

For phase 4, the saved schedule avoids reading partially updated payload; a generation mismatch takes the retirement return before phase/timer writes. The masked arm also excludes interrupt interleaving within its transaction. Neither property protects phase‑1 consumption.

This counterexample requires an old phase‑1 expiry to coexist with another acceptance. The supplied executable source does not establish that this is impossible; omitted admission/masking or priority configuration could exclude it. Therefore the unconditional ownership claim fails this review; an actual reachable firmware failure remains conditional on that missing evidence.
