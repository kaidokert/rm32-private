The raw code shows VREF plausibility checks, an absolute bus-code floor, filtered sag protection, averaged-current verdict handling, foreground fault exits, and ISR shutdown for driver fault, tick gap, stale feedback, campaign deadline, and tracking faults.

Coverage limits matter: phase rails are retained; current protection averages 100 scans and excludes negative residuals. Its final decision logic is truncated here. Sag judges eight-scan means against a moving reference, so it is not raw-dip or slow-droop protection. Fast accepted events can be report-only; tracking does not certify rotor lock. IWDG operation and floor calibration are asserted but not demonstrated by these excerpts.

The supplied screen is **15%, not 25%**. It reports zero LateArm, blank-latch, storm, overrun, and guard faults, plus passing post-stop checks. It supplies no thermal, peak-current, or independent PSU-readback evidence.

**No concrete new defect is established that bars the proposed single 25%, 28-second capped propless reverse attempt after 120 seconds OFF, without retry.** That supports only a bounded screen—not qualification, guaranteed safety, or progression toward 80%.
