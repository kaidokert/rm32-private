# Reverse 48.012 kHz first-peak/host-abort-class diagnostic (retired)

ELF SHA256 `4D286156EE9E3A1E76FC6AEBC267ED6D10F29E07B97D8BFC0BF1E9D0D50164E4`.
Release-hybrid binz opt-s/thinLTO/codegen1, text128884/data1200/
bss17284, four exact motor ISR-root soft-arithmetic audits PASS.
Includes new1333-tick live-duty admission, disabled `livedutycheck`,
and foreground-only host-abort branch classification; otherwise the
same reverse48k/fixed20/active4A/fastbus/nFAULT/tracking/watchdog/
first1900-count peak-stop diagnostic control.

Explicit G071 flash/verify/reset and disabled guard3/18, roledu1006/6
ARR1332, duty6/6, liveduty24/24, p/i off/nFAULT high PASS.
One 115200-baud six-byte burst returned active RX error code0x300 and
stopped safely. Pacing each byte so the command spans ~148ms, below the
250ms parser deadline, produced live ACKs through40%. A protected
40%/60s control completed with no >=1500-count phase excursion,
fastbus, current foldback, tracking, nFAULT or host-abort fault.

One paced gradual climb stopped at the last ACKed43.0% on the first
>=1900-count physical phase sample: IA/IB/IC=1977/4019/1270,
bus/VREF=1188/1506 against start1210/1507, `POWERPATH27` at
45.489254s powered. 43.5% command arrived after the stop and was
refused. No exact ADC rail, fastbus, foldback, tracking or nFAULT.
This is a diagnostic first-event stop, not lean envelope qualification.
Full retained transcript:
`captures/reverse_direction_2026-09-19_48k_peakstop43.txt`.
Exact reverse32k lean SHA06B1C1D6... then restored/verified/reset on
G071 device0x460, p/i all gates/ENABLE/MOE/CCRs0/nFAULT1, COM41 closed.
