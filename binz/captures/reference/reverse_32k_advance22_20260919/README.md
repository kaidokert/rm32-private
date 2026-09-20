# Reverse32k high-duty advance A/B (2026-09-19)

Frozen ELF SHA256
`6FAF3C395C860191AE7B2E0DC682F64B2EED7BCF5A2D17663F68C48026CBD08F`.
Same lean reverse32k current/bus/nFAULT/tracking/watchdog protection
closure as the qualified lower-duty control. Replaces compile-time fixed
advance20 with minz-core's existing scheduled-advance path. Binz publishes
advance level20 through live duty34.9%, and level22 at35% and above;
level20 is seeded at observation begin. This holds the interrupt-mode
low-duty setting equal to baseline and isolates the high-duty2-level
change. Minz-core's compile-time TEMP_ADVANCE is18 in scheduled mode,
so any polling-mode use is a caveat; this is not a byte-identical
startup A/B. Post-stop ADVANCEPROFILE reports the retained level.

Release-hybrid opt-s/thinLTO/codegen1, text123532/data1200/bss17136.
Reverse level boundary host test1/1 and four M0 ISR-root soft-arithmetic
forbids PASS. Not flashed or motor-qualified when frozen. First powered
gate is ordinary10% startup/handoff with disabled guard/role/duty/off
preflight. Do not climb if it fails that gate.

Powered outcome: ordinary10%/15s gate passed with no electrical fault.
One smooth40->45% climb ACKed45%, then fast bus sag stopped the run at
powered67.326891s (`POWERPATH reason=26`), with no current foldback or
tracking event fault. See
`../../reverse_direction_2026-09-19_32k_advance22_45_fastbus.txt`.
Candidate retired; exact fixed20 reverse32k lean SHA06B1C1D6... restored,
verified/reset, disabled checks and p/i off/nFAULT1 passed.
