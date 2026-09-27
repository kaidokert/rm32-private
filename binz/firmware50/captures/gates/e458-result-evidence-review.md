Case 0: 72 × 125 ns = **9 µs**; case 1: 81 × 125 ns = **10.125 µs**, a **1.125 µs** difference. These bracket `after_phase`, not the complete interrupt; neither establishes WCET.

Case 0 matches expected `(requests, phase, armed) = (0,4,1)`. Case 1 reports `(0,4,1)` versus expected `(1,4,1)`: one observation, zero requests, zero retirements. Cases 2–5 have no supplied results. Idle endpoints show enable/MOE clear; both rows report `off=1`. No powered validation occurred.

For average 80 µs, the first due age is **41 µs**. The prepared age is 60 µs; actual observed elapsed is unreported.

Next: repeat only case 1, driver-disabled, recording the **actual values used**: ownership, elapsed, due, line-live, pending, post-level, admission result, and decision. Buffer inside the existing exclusion; report afterward. This distinguishes timing eligibility from live-input refusal without inventing a cause or powering the candidate.
