# E848 aggregate CPU scheduled-advance diagnostic

E846 control/protection plus only aggregate CPU union/root accounting. No
per-event interval ring. Entry/exit clock reads and critical sections are an
observer effect; this image cannot qualify the lean image. A disabled
`cpucheck --union` cost measurement is mandatory before powered use.

- ELF SHA256: `0366DCFF087AF5C8E03CDC8878C337E1AB3EEF26C3BBFB26F099CDFC5D2349F9`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text125576, data1180, bss23224
- four motor ISR-root M0 arithmetic reachability audits PASS
- intended first point: bounded35% hold, not another ceiling climb
