No deadline arithmetic defect found. For used counts 1–4, deadlines are `average + floor(average/2) * used + 1`, matching the supplied rescue predicate’s strict inequality, including odd averages. At `MAX_AVERAGE_US = 10922`, the final deadline is 32767, safely below 0x8000.

- **Ownership limitation:** `Budget` derives `Copy`/`Clone`, so callers can duplicate an active budget and obtain five reservations from each copy. The competing-callers test proves sharing one instance works; it does not enforce one budget per generation. Consider removing these traits and enforcing ownership during integration.
- **Source equivalence limitation:** The supplied existing code admits a fresh request without an elapsed-time condition. It does not establish the new initial `half + 1` deadline; that requires evidence from omitted admission logic.
- **Coverage gaps:** Add rejection checks for elapsed 32769 and `u32::MAX`, plus repeated reservations at one late timestamp. Currently, all five requests can be consumed at that timestamp; quota bounding does not guarantee spacing between actual requests. Tests also hard-code minimum 32 rather than using `AVERAGE_MIN_US`.

The reported tests and identical loadable hash support offline behavior and unchanged binary contents, not atomic reservation/pend, timer invalidation, or hardware correctness.
