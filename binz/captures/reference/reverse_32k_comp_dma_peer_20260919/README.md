# Reverse32k COMP/DMA priority-zero peer A/B (2026-09-19)

Frozen ELF SHA256
`8F3BD75A11BF288BADF53F4D177F71CA984B09A872BDB30D50360211CEF0D052`.
Same lean reverse32k electrical control/protections plus only
`bench-reverse-comp-dma-peer`. COMP joins the existing priority-zero
TIM6 guard and ADC DMA, COM remains priority64. Post-stop readback of
actual NVIC priorities is cfg-gated into this image only. It does not
add powered telemetry or relax TickGap. Release-hybrid opt-s/thinLTO/
codegen1, text123376/data1200/bss17136; all four ISR-root M0 soft
arithmetic forbids PASS.

Flashed/verified on explicit G071, disabled guard3/18, reverse role6/6,
duty6/6, outputs0/nFAULT1 PASS. First ordinary10% startup stopped after
only4 COMs at powered2224us, POWERPATH reason3 (TickGap). Poststop
COREPRIORITY readback `comp=0 com=64 guard=0 dma=0` proves the intended
NVIC map. No high-duty comparison; candidate retired. Outputs off,
nFAULT high. Original reverse32k lean SHA06B1C1D6... was restored,
verified/reset and passed disabled preflights. Capture:
`../../reverse_direction_2026-09-19_32k_comp_dma_peer_tickgap.txt`.
