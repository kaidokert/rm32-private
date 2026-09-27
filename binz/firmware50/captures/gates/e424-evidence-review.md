All 128 displayed ordinals pair correctly, with next-sector identity. Recomputed post-apply timestamp minus acceptance minus requested wait is **5–8 µs**; every fine bracket is **2 ticks = 0.25 µs**. These bracket timestamp reads, not physical switching.

Concrete accounting anomaly: sector accepts sum to **287,894**, which is **1,767 below accepted=289,661**, exactly matching mailbox coalescing. Sector totals therefore cannot independently establish lossless commutation. Also, BEMFTAIL spans **2.278 s**, despite window_us=2,000,000; treating it as an exact two-second rate window is incorrect.

The hold count gives **13,154.7 accepts/s**, or **2,192.4 electrical Hz** under six accepts/cycle. This supports the reported rate, not crossing validity or loaded capability.

The bounded same-image 25% screen can provide diagnostic evidence, but **the supplied join alone cannot discriminate missing COM**: it silently discards unmatched ordinals. Retain and report unmatched accepts/commutations, identify shutdown-censored pending events, and examine acceptance gaps separately. Ordinal attribution also needs scrutiny because COM reads the current acceptance sequence rather than a shown arm-bound identifier.

Nothing here establishes that 25% will pass or supports extrapolation to 80%.
