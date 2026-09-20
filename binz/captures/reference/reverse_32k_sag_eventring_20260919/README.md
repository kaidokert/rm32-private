# Reverse 32 kHz fast-sag event-ring diagnostic

`shell-pwm.elf` SHA256:
`FB6BC15A4574430B8C4D443FB5ADEC8F010E41B29495E0A9B25382FF6689A179`

Release `opt-level=s`, thin LTO, codegen-units=1; text 127100, data 1200,
bss 17428 bytes. This is an opt-in diagnostic image, not the lean
qualification image. It captures three consecutive low-bus rows plus
eight accepted-event timestamps frozen at the first and third low scan.
Fast-bus, current, nFAULT, tracking, and watchdog decisions are unchanged.

Protected 10%/15s and 40%/60s gates passed. The upper-duty attempt ACKed
45%, then stopped on fast-bus reason 26 at powered 44.574632s. See
`../../reverse_direction_2026-09-19_32k_eventring45_decoded.txt`.
Afterward, the exact fixed20 lean image in `../reverse_32k_20260919/`
was restored and outputs-off/nFAULT-high verified.

Correction to a false erratum: `SAGROW ia/ib/ic` are physical IA/IB/IC.
`adc_stream::poll` remaps hardware ADC0/1/4 before this recorder sees
the frame. Do NOT swap archived IA/IC; raw codes and timestamps remain
valid.
