# E819 DRV8304H nFAULT release diagnostic

Diagnostic-only E804 control/protection image plus `bench-driver-fault-probe`.
It is intended to reproduce the retained Driver7 boundary without the later
E817 bus-assisted actuator changing the operating point.

- ELF SHA256: `974AAF816A91FB6F5459EF203068E548F6C75D33535B99313D0E9B68DA2F40D8`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text122316, data1180, bss23172
- on Driver7 only, TIM1 MOE and all six input commands are revoked immediately
- ENABLE remains high only in foreground for a bounded 5.5 ms observation;
  loss of the gate-off invariant ends the observation immediately
- nFAULT is sampled at 100 us, 1 ms, 4 ms and 5 ms; first release time is kept
- ENABLE is unconditionally driven low after the observation
- TI SLVSE39B section 7.3.5 documents 4 ms automatic retry for hardware-interface
  VDS/SENSE OCP, condition-dependent release for UVLO/CPUV/OTSD, and reset-
  required release for GDF
- 34 policy tests in each feature cohort PASS; DMA1_CHANNEL1, ADC_COMP, TIM16
  and TIM6_DAC_LPTIM1 have zero forbidden soft-arithmetic calls reachable by
  emitted direct calls

The diagnostic distinguishes release behavior; it does not identify a unique
fault when multiple DRV protections share that behavior.
