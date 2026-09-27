Raw evidence: 117343 accepts and 117343 commutations, zero forced commutations, zero late arms, and reported maximum COM lateness of 9 µs. These totals do not exclude a terminal ordering failure. `track_fault=1` means **Stale**, not TooFast; `track_max_us=240` is the watchdog deadline, not a measured maximum gap. Tail accepts are already a difference: **17386, without subtracting one**.

**OrderRing is a useful next discriminator, conditionally sufficient to choose a targeted follow-up.** Match accepted and bridge records by ordinal within their overlapping retained tails. A missing bridge or bridge demonstrably late relative to its scheduled deadline supports a COM-path investigation. Timely bridges through the final acceptance followed by silence supports investigating the accepted-event gap. An unmatched terminal acceptance needs its deadline and freeze timing checked before calling it missing.

It cannot establish physical zero crossings, explain rejected or absent acceptances, prove rotor lock, or certify lean-image timing. Software entry timestamps, bracket uncertainty, wraps, and recorder overhead limit attribution; freeze can truncate in-flight logging.

Proceed only through the proposed build/disassembly/audit and bounded low-point screen, with unchanged guards. No higher-duty run on the failed image.
