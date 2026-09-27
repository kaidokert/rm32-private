Raw evidence: both emitted loops reload COMP each iteration. Successful iterations execute 12→8 instructions; the new success branch is taken, so four fewer instructions does not establish four cycles saved or WCET. ADC_COMP shrinks by ten static instructions. Supplied output reports 381 passing tests and clean audits; matching loadable hashes establish equality only between those artifacts.

Claims assessment:

- Predicate equivalence holds: `!(level == rising)` equals `level != rising`. Masking bit30 and comparing against `0`/`1<<30` preserves both polarities; the supplied PAC confirms bit30. Other CSR bits are excluded.
- Each hardware predicate performs one register read, with an emitted load inside the loop. The generic API cannot enforce its “samples once” contract for arbitrary callers.
- Zero depth accepts after blanking with zero reads, preserving existing behavior. The specialized assembly’s unconditional first read is consistent with its positive production depth.
- Tests compare a wrapper against its shared implementation; their independent read-count/acceptance assertions help, but state equality is not an independent baseline regression check. Synthetic words omit unrelated CSR bits.
- Physical equivalence is unproven: changed spacing can accept pulses previously rejected and move acceptance/arming earlier.

No concrete digital defect shown. Evidence supports disabled boot checks, but alone cannot justify the powered screen without peak/thermal stops.
