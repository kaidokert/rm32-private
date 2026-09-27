The capture supports one propless 15% regression run, not qualification or cache efficacy.

- Closed operation: 22.778 s; target hold: 20.278 s. The requested 28 s is not independently verified elapsed time.
- Rates recompute to 12,678 accepts/s overall and 13,114/s during hold. A 76 µs sector implies 2,193 electrical Hz; 74 µs implies 2,252 Hz. Rounded timing explains small discrepancies.
- Tail: 30,095 accepts / 2.277766 s ≈ 13,213/s. This exceeds the declared 2 s window, so it is not a verified last-two-second rate.
- Sector counts sum to 286,955, exactly 1,825 below accepted, matching mailbox coalescing.
- Minimum bus code fell 10.35% from reference; filtered recovery to 99.75% does not erase that excursion.
- Zero forced commutations, reported fault indicators, and safe-off endpoint checks are encouraging. They do not establish continuous safety. Zero margin counters provide no measured margin; 12 µs execution maximum versus 51 µs minimum interval is insufficient.
- Current is a proxy; −256 mA zero drift exceeds the 231 mA hold estimate. Thermal safety is unsupported.

A single protected 25%/28 s screen is conditionally reasonable as further characterization. Verify independent protections and cooling first; 120 s OFF alone is unvalidated. Preserve same-binary verification and no retry/escalation after fault.
