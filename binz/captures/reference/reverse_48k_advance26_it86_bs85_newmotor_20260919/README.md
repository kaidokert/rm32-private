# Reverse 48 kHz advance26 fault-tail diagnostic

Frozen ELF SHA-256 `A074D0E0B9CB0C049CCCB62A14F541B06EF2B03C3B526EC951BB9BC4F35193D4`.
Release-hybrid opt-s binz/thin LTO/codegen1: text127136/data1208/bss18456.
This is the exact protected advance26 ACK-stamp feature closure plus only
`bench-interval-tail` (IT86 last128 accepted intervals) and
`bench-fast-bus-tail` (BS85 last16 coherent bus/current DMA scans).
These add work on the high-rate paths, so this image is **diagnostic only**;
never transfer its pass/fail duty envelope to the frozen lean image.

Four motor ISR-root forbidden soft-arithmetic checks pass. Explicit G071
SN `066CFF343433464757233430` flash verify/reset passed. Disabled-only
guard3/18, roledu100 6/6, duty6/6 and final p/i all outputs off with nFAULT
high passed before a motor command. COM41 was closed before the fixture run.
All current, fast-bus, nFAULT, tracking and watchdog stop policies remain.
