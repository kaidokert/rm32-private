No definite compact-code defect is apparent, but equivalence remains conditional on the omitted `Budget` implementation.

- **Independence:** Legacy retains the budget path, which usefully tests its removal, but shares slot arithmetic and ownership logic with the replacement. Common errors survive. Importing the current `Budget` also means the “frozen” oracle is not self-contained.
- **Coverage:** Every stated valid average, 32 admission patterns, three lateness patterns, repeated timestamps, wraparound, and cancellation boundaries are exercised. This is substantial, but not exhaustive over observation histories: admission bits follow nominal loop slots, arbitrary jitter is absent, and cancellation after prior consumption is not directly covered.
- **Proof gap:** `used <= slot` establishes request-cap safety, not deadline eligibility. Equivalence additionally requires the budget deadline to be no later than the current slot’s due time, and identical validity/cancellation rules. Those cannot be verified from the supplied code.
- **Cost:** 380 cycles = 5.9375 µs at 64 MHz, a 14.51% modeled reduction. Including 24 exception cycles gives 41.296875→35.359375 µs. Attribution is combined; runtime/WCET remains unestablished.

The supplied evidence supports limited differential equivalence and modeled savings, not complete semantic certification.
