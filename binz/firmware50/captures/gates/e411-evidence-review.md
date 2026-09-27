The proposed narrow outcome is supported, with wording tightened:

- **50%:** EFFD1B1C completed a reported **19.778 s target hold**, ending on deadline reason 2. Hold rate 15,108 accepts/s implies 2,518 eHz; the rounded 66 µs interval implies 2,525 eHz, consistent with coast 2,524 eHz. This is one successful propless hold, not repeated qualification.
- **60% request:** fast-sag reason 26, streak 3, **zero target hold**. Terminal CCR 746/1333 implies **55.964% applied duty**, supporting “stopped near 56% during the ramp,” not “failed at 60%.” Zero-hold rate/current fields are unavailable measurements; 166,666 eHz is a denominator-floor artifact.

Counter limitations matter: all six 50% sector counters equal 65,535, strongly indicating saturation; phase bins also hit that ceiling. They cannot establish sector balance or complete distributions. The reported tail spans **3.778 s despite a 2 s window field**, so it cannot be treated as a verified two-second tail. Zero late arms does not prove physical deadline compliance; postblend `ci_min` does not measure the wait estimate.

The wait-dependency change **did not eliminate failure in this single screen**. Unmatched predecessor runs cannot isolate causation. Supply/physical effects and controller timing remain alternatives; current proxies certify neither peak current nor thermal safety.

Poststop checks report outputs off. Next: **offline audit of bridge intermediate writes only**; no powered retry or threshold change.
