# E727 — shorten startup without increasing the current threshold

The previous discussion turn made no implementation progress. This turn read
the full goal attachment and tested a separate `bench-startup-fast` candidate.
The existing startup remains available: 20 ms align, 980 ms catch, 2000 ms ramp,
2000 ms hold, handoff at 4700 ms. The candidate uses 20/100/300/400 ms,
handoff at 520 ms. RUN_TICKS and HANDOFF_US are compile-time expressions from
the stage durations; the same 300 ms campaign tail is retained. Direct-start
fixture mode avoids the old host-side 3.2 s frequency-step schedule.

Build: release, opt-level s, thin LTO, existing E726d features plus
bench-startup-fast. SHA256 of installed frozen ELF:
995a5b51e699ed740c8ce5d933a31158d55244f1f8814440244a2adcc5a9c5be
at captures/reference/startfast_727/shell-pwm.elf. Build/download/OpenOCD reset
all exited zero. Arithmetic audit TIM16 helper gate passed; this is not a
whole-program wide-math or CPU-headroom certificate. Startup policy 25 tests
and seed policy 56 tests passed. Own disabled guard check passed 3/18.

Two direct-start explorations requested a later live ramp to 30%, 15 s BEMF
window, nominal average target800/raw3185 unchanged:

| Startup/drive | Energized | Accepted/commands | Reanchors | Result |
|---|---:|---:|---:|---|
| 8%/8% | 560575 us | 36/48 | 11 | driven reason2, 40 ms seed timeout |
| 7%/7% | 560226 us | 30/48 | 7 | driven reason2, 40 ms seed timeout |

Both STARTUPADC fault0 and AVGDIAG cause0: the earlier average-current stop
did not recur in these shorter campaigns. DMA scans2789/2788. Handler max33us,
overruns0; rate peaks54/59. No BEMF handoff and no target-duty ACK. This is
evidence that shorter timing bypasses the current refusal for these attempts,
not physical current calibration or proof that startup speed follows command.
Both final-off readbacks passed and UART sessions closed.

Capture SHA256:
- startfast_727_start80_direct300.txt:
  5ca3c6a60a2c57a86f856c11e46a56d2e0ec7e320710529f1f26ec2ff8303d51
- startfast_727_start70_direct300.txt:
  87c7f64caa5f845b8bc97797b30e0fd5321eb33ef1ce3ece225d509be8f9e2d0

Next question is the sine-to-six-step normal startup transition: repeated
missing commanded epochs prevent the fresh ordered seed. Do not raise current
threshold or treat optional flying recovery as a gate. Compare the transition
against the earlier working startup path before adding another qualification
layer. Current installed candidate is off, not running-envelope qualified.
