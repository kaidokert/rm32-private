# E799 3.5 A diagnostic exploration candidate

Not flashed or motor-qualified. Board remains E797/2.5A/off pending operator
confirmation that the physical PSU current limit is3.5A.

- ELF SHA256: `4B151FC2660693F68BB5E047E61FCDB6FC1E412C4CBD878E65E411E9F7056A02`
- release profile: opt-level=s, thin LTO, codegen-units=1
- current policy: explicit nominal3.5A, repeatable1% foreground ceiling
- fail-closed rule: a second complete over-limit block before publication and
  acknowledgment remains a terminal current fault
- control:24kHz carrier,226us ADC cadence,advance18,AM32 foreground filter map
- diagnostic output: aggregate current maximum and report-only fast-cycle/event
  fields; no continuous UART stream
- M0 audit: no forbidden helper reachable by direct emitted calls from
  DMA1_CHANNEL1, ADC_COMP, TIM16 or TIM6_DAC_LPTIM1
- size: text121224,data1180,bss23152

After the PSU change is physically confirmed, this is the first image to flash.
Run disabled guard3/18 and all-role2666 preflights, then use the smooth1%/0.25s
ramp for a bounded40% run. Retain any failure and stop if the PSU hard-folds.
This diagnostic artifact discovers the envelope; it cannot qualify the staged
lean E800 image.
