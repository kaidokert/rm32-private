Raw: case6 reports 5 calls, 5 expiries, 5 observations, 0 requests. Times 509/1509/2009/2510/3009µs exceed scheduled 501/1501/2001/2501/3001µs by 8/8/8/9/8µs; gaps are 1000/500/501/499µs. Cases7/8 each report one call: respectively expiry/non-expiry at 1003/6µs. Maxima 64/1/16 ×125ns = 8/0.125/2µs. Case7 skips `after_phase`, so its timing measures no callback. No absolute timestamps appear; host read durations are not firmware latency.

Flags7 means old TIM16 pending witnessed, crossing arm accepted, TIM16 pending cleared; comparator-clear bit is unset. Flags13 means pending witnessed, arm refused, both pending checks clear. All report final phase/armed 0/0 except case7 phase1; all report off/pass.

Interpretation: limited disabled expiry/replacement/stop evidence is supported. Case6 establishes no successful-request delivery latency. Case8 cannot isolate latch protection from other effects of `guard_trip`. Sampled off checks cannot exclude transient output activity. Synthetic ownership, masked ADC_COMP, and diagnostic ISR bookkeeping limit transfer to live timing and causality.

E459’s successful requests/11.5µs and existing guard effectiveness are not demonstrated here.

This packet alone does not justify the powered screen. Next: bridge-disabled successful-request delivery through the production COMP path, including stop/replacement interleavings and end-to-end timing.
