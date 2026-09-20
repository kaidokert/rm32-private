# E806 adaptive 3.5 A aggregate CPU candidate

Not flashed or motor-qualified. Supersedes staged E801 so the CPU image and
lean E805 image share the same adaptive current policy. Board remains E799/off.

- ELF SHA256: `CABD2BB3463E87CEAD66A25A235194DA2E8E69EAFFED4DB79AF6958B3FA1DB20`
- exact E801 aggregate-CPU feature set recovered from Cargo fingerprint
  `binz-677d2c5d2ffa0140`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text124608,data1180,bss23192
- E805 lean control/protection plus only `bench-cpu-aggregate`
- aggregate entry/exit clock reads and critical sections are observer effects;
  disabled `cpucheck` cost must accompany any result
- no event/tail/current-max recorder or continuous UART
- M0 audit: no forbidden helper reachable from DMA1_CHANNEL1,ADC_COMP,TIM16 or
  TIM6_DAC_LPTIM1

E806 measures headroom; its powered envelope never qualifies E805. Do not
flash/power it before the E803 fold is resolved and E804 exploration resumes.
