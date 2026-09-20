# Reverse guarded control, restored after CPU probe

- ELF SHA-256 `629568553F62ED37E2960E12DD0D60E788FFE4370E5715C4C7617C8F3E236092`; `release-hybrid` thumbv6m, binz opt-s/dependencies opt-z/thinLTO/codegen1; text125512/data1200/BSS17176.
- Exact feature closure was read from `target/thumbv6m-none-eabi/release-hybrid/.fingerprint/binz-76378374ecbb2bff/example-shell-pwm.json` (the prior BF06 image). This source rebuild changed ELF SHA after a cfg-gated CPU-only terminal-output edit; image size is unchanged. It is behavior-equivalent by feature closure and build, not byte-identical proof to BF06.
- Reverse A/B phase mapping, normal restart, 24-kHz carrier, real-level revisit, active 4-A signed-current foldback/terminal stop, 5%/three-scan fast bus stop, absolute bus/nFAULT/tracking/watchdog stops retained. Four motor ISR-root soft-arithmetic audit PASS.
- Flashed/verified on explicit G071 SN `066CFF343433464757233430` device0x460, reset. Disabled guard3/18, role6/6 flags127 ARR2666, duty6/6 and p/i all outputs0/nFAULT1 PASS. No powered run on this rebuilt SHA yet; earlier BF06 same feature closure held reverse35/37.5/38.8 but reached an unresolved40% electrical fault.
- This restored image is diagnostic because of its fault-frame/seed-stage telemetry and is **not** a lean qualification image. Do not transfer historical BF06 holds to this exact SHA without a powered control.
