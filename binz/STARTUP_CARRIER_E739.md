# E739 — restore 10 kHz startup and fix averaging phase coverage

Goal read. E738 yielded failure evidence. Startup previously used10k, current
startup20k. Candidate removes bench-startup-20k, retains BEMF20k and all
electrical/tracking/watchdog protections. adopt_startup_carrier now permits
one gate-disabled ARR/UG transition to BEMF20k; ADC uses independent TIM15
TRGO and is not stopped or timestamp-refreshed. No per-COM UG introduced.

Initial start10k_739 SHA8537b00a0e66a7859a10500874a90a4d9bc93aa862865ede6ac9cc4e565a4160
failed averagecause3 during sine at954890us, residual3312>3185. Found a real
coverage defect in this configuration: 50scans*201us visits only half the
100us carrier phase lattice. Changed SCANS to100 for startupADC without
startup20k, preserving50 otherwise. Nominal threshold scales to6371 and
zero scales with same window. Nominal target remains800mA, uncalibrated.
Current response window becomes20.1ms instead of10.05ms; BEMF20k then spans
two complete carrier phase lattices. This fixes integer phase coverage,
not analog aperture/gain calibration or proof that every old trip was false.

Pure startup tests31/31 PASS both default and startupADC100-scan cfg.
Compile-time live assert ties averaging and nominal conversion scan counts.
739b build5eb2cff93e85a6c491b26d8edaf6744c57b769acf6bbb6866cb3b3187f9dcf8d
host aborted before cap1/motor because firmware still printed scans50 with
6371 threshold. Fixed AVGNOMINAL/AVGRAW scan labels; fixture validates either
exact nominal50/3185 or100/6371 pair. Failed cfg quoting attempt retained in
tool output; corrected Python subprocess rustc invocation passed.

Actual/root final start10k_739c SHA256
9aa9609dc7fb0271c887a1dbf1cf248563f4cd3ba009f100743932a32cc27902,
release-s/thinLTO/build/download/OpenOCDreset0/TIM16 mathhelperauditPASS.
Guard3/18 PASS for each installedcandidate. No full sustained qualification.

Capture start10k_739c_start62_drive61_phase60_bemf100.txt: nominal100/6371
ACK, sine completed to handoff, averagecause0/residual-1463. CORESEED now
correctly assumed1/sourcecommanded_startup, interval1666, age264, ARR153,
arm7us. Energized4707943us then tracking stop after handoff. No target300ACK
or sustained BEMF. Final off verified/UARTclosed. This removes the demonstrated
10k averaging refusal, not the remaining BEMF handoff regression.

Next inspect accepted startup sequence/rotor speed versus commanded bootstrap
and reference polling engagement; do not repeat carrier or blind duty sweeps.
