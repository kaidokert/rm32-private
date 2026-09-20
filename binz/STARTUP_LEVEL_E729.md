# E729 — startup level-revisit experiment

Read full goal attachment. Previous turn was progress: exposed phase control,
tested0/30 degrees without rebuild, established phase changes acceptance
distribution but did not restore handoff. This turn inspected current normal
startup and archived E696 working-entry evidence; no archived source snapshot
was located, so no claim of a complete old/new source comparison.

Startup timer previously revisited only an actual deferred early EXTI event.
Added optional bench-startup-level: timer may also wake an unaccepted commanded
sector when the reference half-interval gate is open and its input is already
post-level. It uses existing NVIC retry token; COMP still does normal persistence
and interval sampling. No acceptance, gate write or interval reset in the timer.
sector_accepted clears at command and sets at acceptance to prevent periodic
timer retries after the command has already accepted. Nonowner/software-masked
states cannot gain this new revisit permission. Default adapter remains available.

This is a startup-only level-revisit experiment, NOT a proven AM32 polling-mode
port or proof of the cause of the missing sectors. Both default and new path
still require the existing measured ordered startup seed before transfer.

Release-s/thinLTO build exited0. TIM16 helper audit passed, SHA256
4c30a1271796519a987b6e479c2140deba78b62e0884221c5a8dc00ceb8c8bfc
frozen at captures/reference/startlevel_729/shell-pwm.elf. Download/OpenOCD
reset exited0. Source check4/4 and guard3/18 passed, outputs off. Process-list
command returned exit1 because optional named processes were absent; audit
itself completed and printed its hash. Disabled source check does not test
active-owner timing or the new level admission under drive.

One direct fast8% startup/drive atphase0, requested15s BEMF plus30% live ramp:
energized560228us, driven reason2 at40168us,33accepts/47commands,9reanchors,
handlermax33us/overrun0/ratepeak36. Average diagnostic cause0. No BEMF handoff
or target-duty ACK. Final-off readback passed, UART closed. Capture:
captures/startlevel_729_start80_phase0.txt.

Conclusion: this level retry is insufficient to restore entry. Do not call it
the fix or transfer old20% qualification to this build. Next implement/compare
normal reference startup changeover rather than extending an ordered flying-
seed qualification queue. All electrical protections remain unchanged. Actual
board is this experimental candidate, off; root matches installed ELF.
