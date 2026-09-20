# Reverse32k lower-priority DMA lease A/B (retired after lower-gate failure)

ELF SHA256 `50858F55629E19A7DCEFAD45CE48AB1A4020EFFE479B01FBC74FDB84988500B5`.
Same protected reverse32k fixed20 lean feature closure as the prior
`reverse_32k_dma_below_20260919` candidate; only the DMA snapshot lease
copy deadline changes from 100 to 200 us when `bench-dma-below-comp` is
selected. DMA remains priority128, below COMP/COM64; TIM6 guard stays0.
ADC scans every226us. The 200us deadline remains below that period,
and DMA flags, NDTR half-ownership and epoch checks remain fail-closed.
All other users of `Lease::finish` keep the original100us default.
This tests whether the prior startup `DMAFAULT code7`, service gap237us,
was a scheduling-induced software lease refusal, not whether a high-duty
bus collapse has been cured. The old fault did not retain enough fields
to prove Deadline rather than Position; a failed lower gate will retire
this candidate without a high-duty run.

`release-hybrid` opt-s/thin-LTO/codegen1, text124760/data1200/bss17144.
`dma_snapshot.rs` host tests5/5 pass, including extended-window hardware
ownership negatives. Linker audit on the exact ELF exits0 for all four
motor ISR roots (DMA1_CHANNEL1/ADC_COMP/TIM16/TIM6_DAC_LPTIM1).
The image was flashed/verified and passed disabled guard/role/duty/off
checks. One ordinary10%/15s BEMF hold reached its planned deadline with
no ADC/electrical fault. The immediately following ordinary startup
stopped before the first15% live ACK; the host truncated the terminal
report, so its exact reason is not retained. This is an unclassified
lower-gate failure and the candidate was retired without upper-duty
testing or repeat-until-pass. Exact prior lean image restored and
disabled checks passed. The experimental source changes were removed
after this exact ELF was archived, so future builds use the original
100us DMA lease unless explicitly changed. See
`../../reverse_direction_2026-09-19_32k_dma_lease200_lower_gate.txt`.
