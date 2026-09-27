Raw evidence supplied: PAC extracts bit 30. Both loops contain one register `ldr` per iteration, annotated as volatile. Successful iterations execute 12 versus 8 instructions; the new mismatch path adds an unconditional branch. ADC_COMP totals 763→753 instructions. Tests report 381 passes; four ISR roots pass the audit. Candidate/archive ELF hashes match; the two supplied loadable hashes match.

Claims supported:

- Predicate equivalence holds: `!(level == rising)` equals `level != rising`. Masking with `1 << 30` and comparing against `rising << 30` preserves both polarities and excludes unrelated bits.
- Each hardware predicate invokes the same PAC `.read()` once; emitted loads remain inside the loop. The generic predicate API itself cannot enforce sampling.
- Zero depth performs no reads and accepts after the blanking gate; `count <= blanking` still rejects without sampling.
- Tests exhaust streams through depth 12, but “old” calls the new shared implementation. They independently check read counts and acceptance, not historical state-update equivalence.
- Four fewer executed instructions per successful sample are demonstrated, not four cycles or WCET savings. Hash equality does not establish equivalence to the installed firmware.
- Changed sampling spacing can admit shorter noise pulses. Host tests and ISR audits do not establish physical-aperture safety or powered-test readiness.
