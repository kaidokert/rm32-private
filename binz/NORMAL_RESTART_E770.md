# E770 — reliable ordinary restart on the corrected 50 eHz staircase

E770 fixes a schedule mismatch, not a protection threshold. The autonomous
startup advertised parity with the proven host staircase but began with a
fixed 100 eHz catch, then ramped backward toward 50 eHz before climbing. With
`bench-startup-staircase`, it now begins at 50 eHz and follows the existing
60..200 eHz staircase.

The preceding steps remain part of the result:

- E768 made restart completion explicit, required a fresh powered handoff and
  final reason2, and held all outputs disabled for one second before restart.
- E769 replays the exact first-handoff settings on restart: acquisition 6.1%,
  BEMF 7.0%, phase +60 degrees. Previously the one-shot commands silently
  reverted to 6.2%/6.2%/0 degrees.
- Average current, bus sag, nFAULT, tracking loss and handler/watchdog stops
  remained active. The comparator-dispatch count remains report-only as the
  campaign goal requires.

## Build and preflight

Installed ELF SHA256:
`6DA455B3E090C3A9720245D9D0627DD9BF2594466D180E43587F8F7870AAB272`

Release `opt-level=s`, thin LTO, one codegen unit and the emitted arithmetic
audit passed. The disabled preflight passed all 3 timer-fault cases and 18
post-stop refusals with outputs off.

## Powered cohort

Three predeclared 30 s exploratory campaigns all passed strict validation:

| capture | first tracking stop | fresh restarted hold | result |
|---|---:|---:|---|
| `m0clean_770_direct200_normalrestart_30s_01.txt` | 2,000,905 us | 22,230,005 us | PASS |
| `m0clean_770_direct200_normalrestart_30s_02.txt` | 2,000,905 us | 22,230,011 us | PASS |
| `m0clean_770_direct200_normalrestart_30s_03.txt` | 2,001,005 us | 22,230,004 us | PASS |

Every run established the first BEMF handoff, accepted the injected tracking
loss, disabled the bridge for 1,000,000 us, completed ordinary sine startup,
made a fresh BEMF handoff with `settings_replayed=1`, and ended at the retained
deadline with authoritative powered reason2. Final gates, enable and PWM were
off. Run01 reported a minimum bus of 11.748 V, consistent with the PSU anchor;
no current, sag, nFAULT, tracking or watchdog protection fired after restart.

This is a 3/3 exploratory reliability result for ordinary restart at the
current startup/handoff settings. It is not a higher-duty qualification and
does not justify repeating the same cohort instead of returning to practical
live-duty exploration.
