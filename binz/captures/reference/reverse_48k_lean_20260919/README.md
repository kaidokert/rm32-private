# Reverse 48 kHz lean controller candidate

Frozen `shell-pwm.elf` SHA256
`85DC49F9D66489915FDC742B2E3D68B428CA14233DE30EE7D4041C294577C171`.
Release-hybrid opt-s/thinLTO/codegen1; text124804/data1200/bss17144.
Current-source matched reverse32k lean rebuild SHA
`65A8A7A439F8929BE882951CCECFCD18430782781C328C44B0D1584DC36D9E2C`
(text124760/data1200/bss17144) differs only in the explicit
`bench-reverse-48k` versus `bench-reverse-32k` carrier feature.
The historical installed reverse32k lean SHA06B1C1D6... is a frozen
older build of the same feature closure, not byte-identical to this
current-source rebuild. Both new images pass the four M0 motor
ISR-root soft-arithmetic audits.

No ISR event/scan/CPU recorder, no phase-peak diagnostic stop. Active
nominal4A signed-average current foldback/hard stop, fast >5%-three-
coherent-scan bus stop, absolute bus, nFAULT, tracking, IRQ-rate and
watchdog protections remain. Startup stays at10kHz; BEMF carrier is
48012Hz/ARR1332. ADC cadence remains226us. This is a candidate, not
qualified: disabled hardware checks, reliable startup, lean holds and
normal-start recovery must be measured on its exact SHA.
