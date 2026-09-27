Recomputed: average 80 gives slots **41, 121, 161, 201, 241 µs**; average 81 starts **41, 122 µs**. Late observations skip expired slots without catch-up bursts. The reported COM increase is **146 cycles = 2.28125 µs**; 2385/64 = **37.265625 µs**, excluding exception overhead and unmodelled fetch cost.

No definite handshake defect found in the supplied source: refusal checks and pending publication share PRIMASK; acceptance stops the source and clears TIM16 pending before replacement; stop removes authority.

Two gaps matter:

- `call_max_us` and overrun enforcement run **before** comparator resume and the new wake helper. They do not bound the complete refusal handler.
- `line_live()` checks IMR only. COM can preempt between NVIC masking and IMR clearing at COMP entry and execute optional work. This precedes the decision, but the Boolean model omits that intermediate state.

Required bridge-disabled test: exercise actual TIM16/COMP priorities, CEN/UIF/NVIC pending, and both masking stages. Inject expiry before/during/after refusal resume; verify eventual bounded observation, no duplicate slot request, acceptance cancellation without premature phase-1 dispatch, and stop cancellation. Include hardware expiry while PRIMASK is set.

The audit summaries and abstract tests cannot establish those peripheral transitions or bind the reviewed source to the audited ELF.
