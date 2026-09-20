# E728 — terminal-controlled normal handoff phase

Read the full active goal attachment. Previous turn was progress: short-start
tests removed the average refusal, exposing normal handoff seed timeout.
Installed ELF remains E727 startfast_727, SHA256
995a5b51e699ed740c8ce5d933a31158d55244f1f8814440244a2adcc5a9c5be.
No firmware rebuild, flash, protection or threshold changes this turn.

The live fixture previously hardcoded drive_phase=60. Exposed --drive-phase
with the existing firmware/transport allowed values -30,0,30,60; default60
unchanged. All phase setup occurs idle and requires the existing exact ACK.

Two direct 8%/8% startups used the same 15 s requested BEMF window and later
30% target ramp. Neither reached BEMF or acknowledged the target:

| Shift | Energized us | Accepts/commands | Reanchors | Result |
|---|---:|---:|---:|---|
| 0 degrees | 560574 | 30/48 | 9 | driven reason2, 40 ms seed timeout |
| 30 degrees | 560609 | 29/48 | 14 | driven reason2, 40 ms seed timeout |

Both average diagnostic cause0, handler max33us/overrun0, final-off readback
passed, UART closed. No claim of calibrated physical current or sustained lock.
0 degree retained first26 acceptance rows contain steps4,5,6,1 but no2/3.
30 degree retained first26 contain steps1,2,4,6 but no3/5. These are bounded
early acceptance traces, not whole-run phase coverage. The changed distribution
shows acceptance depends on transition phase; none of 0/30/60 established
the six ordered intervals required by this startup adapter.

Artifacts: captures/startphase_728_start80_phase0.txt and
captures/startphase_728_start80_phase30.txt. Next compare the normal startup
adapter with the old working entry path: do not turn the phase search or
optional flying recovery into another certification queue. Evidence does not
point to wiring or a hardware speed limit. Actual outputs off, E727 installed.
