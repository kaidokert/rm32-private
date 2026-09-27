- **Shutdown dominance remains unproven.** Cases 1–3 pend the guard inside PRIMASK, forcing shutdown after the transaction. They do not test a guard trip before entry followed by resumed COMP code that could rearm COM. Final snapshots also cannot exclude transient rearming.

- **Interrupt ordering needs stronger evidence.** Setting guard priority alone does not establish its relationship to ADC_COMP and TIM16. Case 4’s zero COM calls may demonstrate pending-bit cancellation; it does not exercise the production COM handler or prove guard dominance over simultaneous dispatch.

- **Off-only containment is conditional.** `off()` checks enable, MOE, and CCRs, but excludes gate-pin state. The added `p` readback occurs after another `safe_off()`, potentially hiding a containment failure. Initialization and failure paths also need scrutiny.

- **The input change is plausible, not demonstrated.** VREFINT selection plus delay does not establish reference readiness or comparator stability. Selecting the expected edge from one sampled level makes acceptance partly self-calibrating. Case 2 tests deliberate polarity disagreement, not noisy-input rejection.

- **Evidence does not match the proposal.** E473 failed case 0 and used different telemetry; E474’s structural audit proves neither runtime behavior nor electrical safety. Require exact-image verification and fresh five-case results. Unsafe vector/priority contracts remain assumptions until initialization and effective priorities are checked.
