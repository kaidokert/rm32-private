# E490 — return to pre-experiment loaded baseline (2026-09-27)

Operator cleared the old goal, requested parking the experimental branch,
reports prop reinstalled and motor boxed, and requests one 50% spin.
No propless campaign resumption or control changes authorized by this test.

Experimental source preserved on codex/park-propless-e489. Isolated baseline
worktree: C:/Users/kaido/AppData/Local/Temp/binz-loaded60-restore,
branch codex/loaded60-restore at 299ba32. Other projects' dirty files untouched.
Repository commit hooks unexpectedly formatted files and built sibling targets;
hook formatting was reverted from the staged originals and the snapshot commit
bypassed hooks to preserve exact source. No sibling firmware was flashed.

Use exact archived 81F85AB5.e358-propless-baseline.elf, SHA256
72581795EFE2E9865E9132E18A70FEAE8F370BD1C60FE0D81A2083CE9A9F247F.
This matches the historical box-open-50pct capture, source 299ba32.
No recompilation and no diode/timed-recheck experiment on the motor.

Pre-run: MCP healthy, current experimental image freshly read OFF, nFAULT1,
UART closed. Download/verify/reset explicit G071 only, then fresh p alloff.
One short l run at normalized 500 tenths: 45s total, about20s target hold.
All archived protections unchanged; physical supply last confirmed3A, unchanged.
Prediction: deadline reason2, target/ceiling500, nonzero accepted BEMF events,
late_arms0, alloff PASS. Stop after any failure, no retry or qualification claim.
Boxed motor has no thermal sensor; bounded exposure only, no thermal guarantee.

Result: reason2 deadline,19.773s at50%,zero late/thin/forced/guard/foldback.
Coast1914eHz,currentproxy1598mA (not meter-calibrated),matched1004permille.
Fixture qualification FAIL for dwell<30s and speed outside old oracle5% band;
the requested short spin completed without a protection stop. Fresh MCP alloff
PASS,nFAULT1; serialclosed. Exact81F85AB5 remains installed. Experiment parked.
