# E805 lean adaptive 3.5 A qualification candidate

Not flashed or motor-qualified. Supersedes staged E800, whose fixed1% current
actuator was falsified by E803. Board remains E799/off; do not power this image
until the physical fold is resolved/corroborated and the operator resumes.

- ELF SHA256: `39FF82CD266582473B2FFEAC99578273D46EEA712BD39C83DD6E23C585AE03CE`
- exact E800 lean feature set recovered from Cargo fingerprint
  `binz-5e5e9a8b48f25cac`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text120672,data1180,bss23124
- same adaptive1..5% current actuator and unchanged safety thresholds as E804
- intentionally omits average-current maximum, fast-cycle/event and tail
  diagnostics; retains lean-core and lean-irq bodies
- M0 audit: no forbidden helper reachable from DMA1_CHANNEL1,ADC_COMP,TIM16 or
  TIM6_DAC_LPTIM1

Only this image may eventually qualify10/25/50% holds and normal-start recovery.
Runtime still requires the strict lean verifier; build composition alone is not
qualification.
