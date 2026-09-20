# E801 aggregate CPU diagnostic candidate

Not flashed or motor-qualified. Board remains E797/2.5A/off pending operator
confirmation that the physical PSU current limit is3.5A.

- ELF SHA256: `CA33BD31B860DA7EF65A9E37F100C9B7F3341D10B5AD462BCCCBEF5B117A40C3`
- release profile: opt-level=s, thin LTO, codegen-units=1
- control/protection matches staged E800 lean3.5A candidate
- adds only `bench-cpu-aggregate` (`cpu-roots`/`cpu-union`/`cpu-timing`)
- retains lean-core and lean-irq control bodies; no event/tail/current-max recorder
- aggregate RAM counters account nested IRQ union/root time and dump post-stop
- no continuous UART and no per-event buffer
- M0 audit: no forbidden helper reachable by direct emitted calls from
  DMA1_CHANNEL1, ADC_COMP, TIM16 or TIM6_DAC_LPTIM1
- size: text124332,data1180,bss23188 (lean E800 text120396,bss23120)

Every vector entry/exit gains accounting and clock reads, so measured occupancy
is observer-affected. Before a powered run, execute the disabled `cpucheck` and
retain its software-pair cost; do not transfer the diagnostic image's envelope
to E800. Runtime output must have a sound CPUUNION/CPUROOT snapshot and no meter
fault. This image exists to measure timing/headroom separately, not qualify50%.

