# Reverse32k fixed-advance18 control A/B (2026-09-19)

Frozen ELF SHA256
`1B43148BE87CBD8F0E665169D6B68F28B524BC10FEA8125E679530D57722F3AC`.
Built with the exact feature list from fixed20 reverse32k lean fingerprint
`binz-a45d423fa4537bdc/example-shell-pwm.json`, replacing only
`bench-advance-20` with `bench-advance-18`. This changes the minz-core
fixed advance constant; reverse phase mapping, 32k BEMF carrier, 10k
startup carrier, 226us ADC stream, active4A signed-current foldback/stop,
fast5%-three-scan and absolute bus stops, nFAULT/tracking/watchdog/IRQ-rate
protection and normal restart remain in the feature closure.

Release-hybrid opt-s/thin-LTO/codegen1; four motor ISR-root transitive
soft-arithmetic audits (`ADC_COMP`, `DMA1_CHANNEL1`, `TIM16`, `TIM6_DAC_LPTIM1`)
PASS. This image is staged only, NOT flashed or motor-qualified at freeze.
The installed board remains fixed20 reverse32k SHA06B1C1D6..., OFF, COM41
open after the clean42.5% run. A prospective powered A/B must pass
flash verify, disabled guard/role/duty/off checks and an ordinary10%
startup/handoff gate before testing42.5/45; retain the first fault.

Powered outcome: ordinary10%/15s gate passed. The first smooth climb
ACKed42.5% but stopped at35.148728s powered on the active fast relative
bus guard (`POWERPATH reason=26`), with no current foldback or tracking
event fault; no45/50 attempt. Capture:
`../../reverse_direction_2026-09-19_32k_fixed18_425_fastbus.txt`.
Candidate retired. Exact fixed20 reverse32k SHA06B1C1D6... was
reflashed/verified/reset; disabled checks and p/i off/nFAULT1 passed.
