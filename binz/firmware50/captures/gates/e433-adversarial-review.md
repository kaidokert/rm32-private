The evidence supports retaining E433 as an offline candidate. It does not establish full behavioral equivalence or stop coverage.

- COMP still publishes accepted estimates before timer arming, subtracts elapsed time after timer preparation, branches to `stop_expired_arm`, and retains guard-triggered shutdown writes. TIM16 retains inhibit checks around bridge updates. No obvious guard removal appears in the supplied bodies.
- Constant wait emission is `floor(avg/2) − floor(avg/4)` at `0x08000978–0x08000990`, using the freshly updated, clamped average. Preserve this rounding explicitly; it is not always `floor(avg/4)`.
- Supply `stop_expired_arm` and `comp_exti_arm` disassembly plus the audit rules. “Reachable=2 / OK” alone cannot establish those helpers’ stop and rearm behavior.
- Supply the `ChainLog::ORDER` default and an annotated old/new comparison covering both roots and recorder lifecycle effects. New assembly and instruction totals alone do not demonstrate that every removed operation was recording-only.
- The 380-test summary establishes a passing host suite, but does not identify the claimed 1000/1001 cases or connect its configuration to this ELF. Record complete build/test commands, features, revision, and artifact hashes.

Matching hashes establish archived-image identity. Instruction reductions are not WCET evidence. These gaps limit offline sign-off; they do not establish a defect.
