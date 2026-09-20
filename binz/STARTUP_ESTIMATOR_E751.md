# E751: dedicated normal-startup estimator, not flying per-edge qualification

Goal read. E750 yielded evidence that the borrowed flying1ms per-edge bound
resets commanded startup on late/early compensating pairs.

New with_startup_estimator mode selected ONLY for bench-startup-adc:
twelve measured contiguous-sector intervals, original40ms campaign deadline,
actual TIM2/bracket corroboration and sector order, missing epochs clear the
window instead of contributing averaged gaps. Zero spacing, spacing>6ms,
invalid order or timer/bracket mismatch still refuse. Individual cycle/spacing
quality no longer borrows the flying interval floor/ceiling. Final mean must
remain within existing SEED_MIN_TICKS..2000 seed geometry before arm.
Six-entry onset history reports seven full-cycle measurements. No fabricated
edge, synthetic COM, timer-age refresh, or electrical-guard change.
Existing strict/flying constructors unchanged.

Arithmetic: twelve bounded intervals produce sum<=24000 at accepted seed.
Mean=(sum*21846)>>18, 32-bit product<=524304000. Exhaustive0..24000 equality
to sum/12 tested; no runtime division or64-bit product here. Index wraps with
branch, not modulo. Three new tests cover compensating2200/1132tick intervals,
full-cycle results, missing epochs, corrupt timer, expired window and u32 wrap,
plus frozen completed seed. 60 seed-policy tests PASS total.

Frozen/installed captures/reference/estimator_751/shell-pwm.elf SHA256:
87E07DAB6816045E97ADD33537E40808825AA67FFD77181B21790588DAD38EB0.
Release-s/thinLTO/build0/TIM16helperaudit/download/OpenOCDreset0;
own guard3/18 PASS. Diagnostic startup records remain, not qualification image.

Motor test estimator_751_start100_bemf150.txt:
direct100->200 sine40->100, forced100/phase60, initialBEMF150/requestlive300.
Startup release11098us,13commanded sectors/14accepts including partialepoch0;
measuredseed1670/edgeage196ticks/ARR222/arm7us, reanchors0, handler18us.
Twelve measured interval seed now succeeds instead of E750b timeout.
Powered commits2, latest accepted345us/sector4, tracking stop1405us,
ADC1223us fresh, averagecause0. No300ACK/sustained15% or lock qualification.
Final-off verified/UART closed.

Conclusion scoped n=1: eliminates the demonstrated startup per-edge window
refusal in this test, but NOT the subsequent BEMF handoff/tracking failure.
Next examine post-handoff sensing/commutation chronology with this estimator,
not more flying-recovery cohorts or treating seed completion as lock. Normal
restart, current calibration and representative sustained qualification remain
unfinished. Goal active; no hardware-limit/CPU headroom claims.
