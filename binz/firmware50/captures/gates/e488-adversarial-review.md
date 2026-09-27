The 15% run supports a completed propless screen: 20.277s at target, deadline stop, zero forced commutations, zero late arms, and passing pre/post bridge-off checks.

Numerators reconcile: sector accepts total 243,684; adding 48,364 mailbox-coalesced accepts gives 292,048. Hold accepts are a separate 262,607. Tail identity uses 29,539 accepts over 2.277110s—not the nominal 2s—giving about 2162 electrical Hz versus reported coast 2167Hz (998‰). Tail origins reconcile exactly, ending 150µs before stop, within the 666µs reported service/tracking budget. The displayed coast intervals cover only the beginning; they cannot independently reproduce the full matched-coast calculation.

Current is uncertain: −253mA zero drift is comparable to the 257mA hold proxy. Neither that proxy nor the 1120mA reported worst value establishes physical current headroom.

One whole-run thin event cannot locate a phase or establish a failure rate; zero margin histograms add no evidence.

I see no concrete raw-evidence reason to decline **one narrowly admitted 25% screen** under the stated same-ELF, ≥120s OFF, physical 3A, propless/reverse, 28s total/14s target, no-retry-after-fault constraints. Keep late_arms=0 mandatory. This establishes neither a causal cure nor new qualification.