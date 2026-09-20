# E762 — representative lean qualification at 15%

The qualification image enables lean-core and lean-irq. COMP/COM event,
maximum-time and accepted-event recorders are absent; the accepted-log header
is present but reports n=0 and has no AE85 rows. DMA timing/count diagnostics
and average-current diagnostics are absent. Functional guard work remains:
signed average-current scan, bus/nFAULT/tracking, coherent DMA publication,
commutation writes and execution watchdogs.

Frozen image `captures/reference/quallean_762/shell-pwm.elf`, SHA256
`b216931bed6f786dd182c98f62fd8fad13e2a68c54cf64388c83613bf7341b00`.
Release opt-s/thinLTO/codegen1, direct TIM16 arithmetic audit, download,
OpenOCD reset and disabled guard3/18 passed.

Three predeclared same-image runs used autonomous startup, live ramp10→15%,
and a30s powered window. All three completed on deadline reason2, had no veto,
fresh tracking and feedback, final outputs off, and UART closed:

| capture | stop us | commits | last-event age us | feedback age us |
|---|---:|---:|---:|---:|
| `quallean_762_direct200_ramp15_01.txt` | 30,000,005 | 121,265 | 187 | 120 |
| `quallean_762_direct200_ramp15_02.txt` | 30,000,004 | 120,935 | 134 | 120 |
| `quallean_762_direct200_ramp15_03.txt` | 30,000,005 | 120,669 | 231 | 121 |

`scripts/verify_lean_qualification.py` checks compact-frame CRCs, exact lean
markers, absent/nonpopulated recorders, live duty ACK, deadline outcome,
control progress, freshness, no veto and final safing. It requires at least
three retained runs. The cohort passes.

This qualifies one representative operating point, not the full0–30% envelope.
The lean image deliberately cannot report qZC or interval sigma; those belong
to separately hashed diagnostic images. Current protection presently bounds
the sustained envelope below30% on the1A bench setting.
