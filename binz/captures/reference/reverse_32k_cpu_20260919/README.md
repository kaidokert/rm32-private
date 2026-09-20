# Invalid comparison image — never flashed

ELF SHA256 `D288B528D0FFB3DDB441D420910DC868DBA8D01CED81F896586570095409F46F`.
This first CPU build omitted `bench-dma-peer` from the frozen lean
feature closure, so it would change DMA priority as well as adding
instrumentation. It was caught by fingerprint comparison before flash
and must not be used as a one-variable CPU probe. The corrected image
is in `../reverse_32k_cpu_peer_20260919/`.
