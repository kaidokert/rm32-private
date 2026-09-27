**The diagnostic confirms a sampled terminal sag, but does not establish its physical cause or a 57% lean limit.**

- **Actual exposure:** E400 lean reached70% and held9.025s before reason26. E402 diagnostic completed normally with19.778s at50%. E403 requested70% but stopped at **57%** (`CCR570/period1000`, matching every tail row), with zero target dwell. Its166666eHz hold estimate is an empty-window artifact. None establishes a qualified rung; all supplied poststop checks show outputs off.

- **Guard arithmetic:** E403’s final reference is1212/1507; final judged VREF is1507. The threshold is therefore1151.4 bus codes. Means1134,1123,1112 equal **93.564%,92.657%,91.749%** of reference. Their cross-products170893800,169236100,167578400 are each below173515980, correctly producing streak1/2/3. The preceding1152/1507 remains just above threshold. This is consistent guard behavior.

- **Sampled chronology:** At timestamp937, phases2119/4094/**0** accompany raw bus1169, still above95% of reference. Subsequent rows contain4095 and repeated zeros. First terminal raw sub95% sample is1097 at1335, approximately398µs after that first rail; latch follows approximately1111µs after it, using fine stamps. Final streak spans200.375µs. Rails precede the sampled threshold crossing, **not necessarily physical sag onset**. They indicate clipping, not calibrated amperes; neither PWMctr nor foreground accept age supplies aperture-synchronous causality.

- **Reference versus notch:** Supplied slow endpoints show reference1213→1212 with nearly unchanged VREF; the fast tail reference stays1212/1507. That small evolution cannot explain the terminal depression. Missing middle slow rows preclude a complete reference-history audit. Current averages also cannot exclude a terminal surge.

Retiring the recorder for envelope work is justified: its masked copies can perturb timing despite unchanged IRQ roots;48µs is not measured WCET. Lean’s failure means recording is not necessary for failure.

The80kHz experiment is a reasonable conditional A/B, with two corrections: **800 ticks requires a selftest acceptance window around12.5µs; retaining14–17µs would reject it.** Also, lean cannot measure predicted raw-low clustering. Keep the proposed bounded60→conditional70 sequence and unchanged guards; completion would support further testing, not establish thermal benefit or80% qualification.
