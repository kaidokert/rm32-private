# E804 adaptive 3.5 A diagnostic candidate

Not flashed or motor-qualified. The board remains safely off on installed E799
after E803's retained bus-sag stop. This artifact must not be powered until the
operator resolves/corroborates that power-path fold and explicitly resumes.

- ELF SHA256: `389B0BFE9AC8848C849B891F32883BC0B246DE85DF963312D4BB8F9540A8D10D`
- exact E799 diagnostic feature set recovered from Cargo fingerprint
  `binz-58cf1a3b2f4e44a6`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121492,data1180,bss23156
- unchanged protection thresholds: nominal3.5A signed block current,8.4V
  complete-block bus floor,nFAULT,tracking,watchdog and second-over terminal stop
- changed actuator only: first-over residual is retained; foreground chooses a
  division-free1/2/3/4/5% reduction at <=110/125/150/175/>175% of allowance
- warning magnitude latches the worst pending complete block; DMA still has no
  PWM authority and foreground publishes/acknowledges the coherent lower ceiling
- post-stop summary reports the actual final step and adaptive policy
- host policy/wire tests PASS; actual policy modules20/20 PASS at30% and50% feature
  ceilings; average-current policy9 tests PASS
- M0 audit: no forbidden helper reachable by direct emitted calls from
  DMA1_CHANNEL1,ADC_COMP,TIM16 or TIM6_DAC_LPTIM1; the new record path has no
  `__aeabi_uidivmod`

E803 measured1.853x nominal residual. Under this policy that warning requests
the bounded5% maximum rather than the disproven fixed1%. This is a causal
protection response, not permission to rerun through an unresolved supply fold.
