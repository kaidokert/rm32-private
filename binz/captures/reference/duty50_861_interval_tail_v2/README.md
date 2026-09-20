# E861 decision-input interval tail

Installed ELF SHA256
`8CA10D96ACFF5C9CA00F64B9E0241084AD222B1F516EF92713C871EBE97CBCF4`.
This is the exact E857 release feature closure recovered from Cargo's frozen
fingerprint, with the interval tail upgraded from IT85 to IT86-v2.

Each retained accepted event now records the TIM17 gap, EV_ACC measured
interval, the actual foreground `S.average_interval` passed to the speed-scaled
watchdog, and the watchdog deadline in force before that event. Dumping remains
post-run only. The hot path adds one diagnostic atomic mirror store and two RAM
stores; Recorder grew16 emitted bytes. Relative to E857: text122212 (+152),
data1188 (+8), bss24488 (+512). Full disassembly has no arithmetic helper in
Recorder or transitively from ADC_COMP, TIM16, TIM6_DAC_LPTIM1 or DMA1_CHANNEL1.

Host tests: speed-watch4 PASS; IT85/IT86 decoder/transition5 PASS. Exact linked-image SHA,
feature closure and disassembly are frozen here. Flash/verify selected ST-LINK
`066CFF343433464757233430`, device0x460. Disabled hardware checks passed:
guard modes3/3 with post-stop refusals18/18; role steps6/6 at2666 ticks. Final
`p` and `i` showed all gate outputs/inputs, ENABLE, MOE and CCRs low/zero and
nFAULT high. UART was closed. No motor run or powered hold was made because the
E855 hard-fold pause remains.

This image is ready for a known-clean control only after that pause is resolved.
It is not powered-qualified and says nothing new about the duty ceiling.
