# E731 — two stale startup assumptions fixed

Read full goal attachment. Prior E730 passed seed gate but refused powered
transfer. Added foreground-only first-refusal reason in adopt_driven, preserving
check order/short-circuit semantics; post-stop STARTUPSETUP prints already stored
stage ages, no new ISR recorder. First two diagnostic images still refused
before coast setup. Inspection found DUTYSPLIT acquisition80/bemf0: helper
select still admitted acquisition40..62 while new timed startup admitted40..100.
Added timed select_startup40..100, retained old defaultselect40..62 and BEMF
initial duty<=100. It now preserves requested BEMF70 for startup80.

That fix reached a real panic. split_731c session timed out, final UART off
verification failed, and host closed. No overlapping UART/SWD. Debugger halt
read PANIC_LINE at20000484=0x24 (36), PD1 ODR=0, TIM1BDTR=0xc1a (MOEoff).
Line36 phase_role_live::prepare_carrier rejects active ADC DMA. This is an
ownership assumption incompatible with uninterrupted timed-startup ADC,
not a motor speed/hardware limit. Do not claim UART off verified on this image.

Added adopt_startup_carrier: for explicit timed startup initial transfer only,
validate actual carrier ARR, zeroCCR1/2/3 and outputs disabled/no powered owner;
retain period without UG or disturbing ADC DMA. Other paths retain prepare.
Final carrieradopt_731d build release-s/thinLTO/TIM16auditPASS, SHA256
0422884a2c598b65deb8ceaa11c53cb5b2df3cf4b5733ac71006057207a8df08.
All candidate builds/downloads/OpenOCD resets exited0.

Final directfast8%/8% phase0 test, later30% target requested: released startup
reason22 at5244us,6accepts/6changes; adoption passed; timer ARMED atedgeage238
half-us ticks, remaining179ticks, arm6us. Existing summary labels this measured
flying, but that label is WRONG for bootstrap: actual seed is commanded1666,
explicit STARTUPBOOTSTRAP marker remains authoritative. COASTREF stop7 and
POWERPATH reason8 show tracking refusal after1405us; accepted monitor stale,
zero powered accepted intervals. No sustained BEMF/target ACK. Averagefault0,
FIRSTFEEDBACK seen0. Final UART off readback passed and session closed.

Artifacts adopt_731_start80_phase0.txt, setup_731b_start80_phase0.txt,
split_731c_start80_phase0.txt (FAILED timeout/off verification),
carrieradopt_731d_start80_phase0.txt (clean stop). Current/root final731d off.
Next startup tracking and producer handoff: why zero fresh powered deliveries
and no next qualified event after first COM. Do not restart phase grind or
flying qualification. This turn restored timer arm, not sustained running.
