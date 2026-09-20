# Reverse high-duty level-revisit A/B — retired after powered35% failure

- ELF SHA-256 `9D46A9B8AAFF128DE036489502E741E8CBCAF0B8092DC49F0035F3DFB0FA9CC8`, `release-hybrid`, text125644/data1200/BSS17176.
- Exact BF06 reverse-control feature closure plus `bench-running-revisit-off35`. Below 35% the established foreground one-pend, real-level, real-persistence rescue remains; at ACKed duty >=35%, foreground will not pend it and only physical COMP edges may accept. Current/fast-bus/absolute-bus/nFAULT/tracking/watchdog stops are unchanged. Source decision is serialized with duty publication; no new ISR work.
- Pure cutoff test passed at 34.9%, 35%, and 50%; four motor ISR-root soft-arithmetic audit PASS. Explicit-G071 flash verification/reset and disabled guard3/18, role6/6, duty6/6, p/i passed.
- Powered result: ACKed35%, then Tracking8 at10.295059s; automatic ordinary-start recovery restored35%, then another Tracking8 at10.112055s. FASTBUS0/foldback0/rail0/bus-low0; p/i all outputs0/nFAULT1 after stop. Capture `../../reverse_direction_2026-09-18_off35_tracking.txt`. `REVISITPOLICY` witness was emitted. This candidate is retired; 37.5/40 were not attempted.
