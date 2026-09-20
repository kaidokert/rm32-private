# Reverse 32 kHz fast-sag pre-trigger scan-ring diagnostic

`shell-pwm.elf` SHA256:
`E33CBCA1A57E34D08DFA55F6EF7D8B4DC001BDDD887FC6AA277A44850E03D9BC`

Release `opt-level=s`, thin LTO, codegen-units=1; text 128060, data
1200, bss 17632 bytes. Four exact motor ISR-root soft-arithmetic
audits passed. This opt-in diagnostic adds eight DMA-owned coherent
raw-ADC scans frozen on the third low-bus sample to the V2 three-row
snapshot and accepted-event timestamp windows. It changes no stop or
control decision and is not the lean qualification image.

Protected 10%/15s and 40%/60s gates passed. Three same-image ramps
ACKed 45% and all stopped on the existing fast-bus reason 26. Full
captures: `../../reverse_direction_2026-09-19_32k_scanring45[a-c]_fastbus.txt`.
The exact fixed20 lean image in `../reverse_32k_20260919/` was
restored and outputs-off/nFAULT-high verified afterward.

Archive caveat: this frozen image prints `SAGCAUSE fault_triggered=1`
even for an isolated benign low scan in a deadline-completed run. The
post-archive source corrects that post-run label; the powered cohort
interpretation uses `POWERPATH reason` and `FASTBUS tripped`, not the
static label. The source-only label correction was not flashed.

Correction to a false erratum: the archived `SAGROW`/`SAGSCAN` labels
already are physical IA/IB/IC. `adc_stream::poll` remaps the hardware
ADC0/1/4 sequence through `dma_snapshot::logical` before this recorder
receives a frame. Do NOT swap archived IA/IC. A mistaken offline-only
source swap was built as SHA `9BEE913C...` but NEVER flashed and was
reverted. Raw codes, timestamps and phase labels in this frozen image
are valid.
