# Fresh admission, in-place guard installation — E366

## E369 preceding higher points

Same957F, all guards unchanged. guardinstall_hold65_01 andhold66_01 PASS10s
299.918/304.180eHz, IRQ54.838/55.057%, raw264/250bus11116/10925mV.
guardinstall_reentry65_30s_01 and66 PASS original30s with fresh recovery:
299.765/304.363eHz, IRQ55.283/55.490%, sigma29.211/28.785us.
Both have total seedage91us, guard-start15us, armcost12us. Actual arm50/48us
(18/16us above32floor), versus archived58B57044.5/43us at preceding points.
Observed COMP65/COM69/commit45us, stack2696 each; no WCET claim.
Raw running278/247,bus10984/10769. Startup peaks for hold65/recovery65/hold66/
recovery66 are513/511/974/424; retain high startup peak separately, notamps.
Full fixture/provenance/deadline/finaloff verified; one hold/recovery each,
not a new repeatability cohort. Next coordinated profile/arm-budget audit;
the improved entry cost does not prove reduced same-phase timing variation.

## E368 recovery repeatability

Installed957F unchanged. Fixed3/3 original30s recovery cohort PASS, full
chronology/CRC/guards/provenance/deadline/finaloff verified. Manifest:
captures/guardinstall_reentry64_30s_cohort.csv. Added02 and03 recover at
292.861933/292.728466eHz, IRQ54.769142/55.282941%, sigma24.830430/24.555539us.
COM/accepted49183/49183 and49161/49160; raw275/290counts,bus10937/10865mV.
Startupraw449/519 is separate. COMP65/COM69/commit45us, stack2696 each.
Recovery arm53/52.5us, cost12us, original deadline spare206/208us.
All three have SEEDLAT49/10/7/15/2/8us=91us, versus archived96us/guard20us.
This demonstrates repeatable favorable handoff timing at this point, not
masked WCET or steady-state IRQ reduction. No higher duty qualification yet.
Next preceding65/66 hold/recovery; avoid further equivalent64 repeats.

E367 installed; one64hold and one64recovery PASS. Prior58B570 archive:
captures/reference/staticcomp_58b5/shell-pwm.elf. Installed
957F6C5A1CD9F78CE818141263571500299B27C05A4492882A23E64556947F3F.

Optional bench-guard-install splits fresh validation from materialization.
RunGuard::admit validates profile, campaign, segment, seed, current/bus/VREF,
then initial feedback age in legacy refusal order. It returns a private-field,
non-Copy/non-Clone, profile-specific Admission token with original timestamps.
No token storage/global prestaging or refreshed clock. In start_inner it is
consumed immediately inside the existing masked installation section. install
constructs the destination guard, then the unchanged caller initializes clock,
sets ACTIVE and enables TIM6/NVIC. Prior ownership/ready/off checks unchanged.
The token is policy data, not hardware authority; it must never be retained
across ownership/time transitions. There is no new unsafe implementation code.

Default with_limits/age API stays unchanged. New tests compare refusal precedence
over valid/invalid step, feedback, campaign, segment, age and wrapping origins;
installed timestamps/poll outcomes and event histories match legacy behavior.
205Rust tests pass with feature,306Python tests pass. --guard-install strictly
requires GUARDINSTALL admission_token=1 fresh_checks=1 in_place=1 prestaged=0.

Pre-marker assemblyDEDBCB: start_inner localstack52 vsbaseline180bytes.
Installer uses24byte memset for cycle history and field stores, no fullguard
memcpy. Its localstack28bytes and extra masked initialization require hardware
measurement: reduced copies do not prove reduced masked duration. Finalmarker
build text121628(-128vs58B570),data1104/bss29324, release/s/thinLTO/codegen1PASS.

Next finalassembly review, disabled priority/pulse/atomic/roles/CPU/archive and
guard-refusal checks before power. Then matched64hold10s/recovery30s with
--static-comp --guard-install (NOT --inline-guard). Require all original full
fixture/provenance/electrical/age/32usarm/16usarmcost/deadline/finaloff gates.
Compare SEEDLAT to baseline96us total/20usguard and failedinline102us/26usguard.
No67retry or profileincrease before demonstrating useful safe timing behavior.

## E367 hardware result

Finalassembly retains directfieldstores and24byteclear,no fullguardcopy.
priority01UARTsilent retained; RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 verified
beforeUSARTclock08040000 repair. priority02threePASS; new drv_guard_check.py
validates three timerfaults (feedback4/tracking8/deadline1) and18poststopwriter
refusals withENABLElow. preflight01all5PASS CPU2/7/10us.309Python testsPASS.

guardinstall_hold64_01 PASS10s292.859510eHz17572COM/17571acc,IRQ55.054866%,
sigma35.042979us,COMP65COM69commit45,raw276bus10877,arm72.5cost13stack3356.
Startupraw737bus11175. SHA790E1C3CD1EF4F252233821036DF367DCB2D48D0C2447BC1BB868A126751B68A.

guardinstall_reentry64_30s_01 PASSoriginal30s292.870297eHz49185COM/49184acc,
IRQ55.246208%,sigma25.076083us,COMP65COM69commit45,raw267bus10817stack2696.
Seed1163/acq7416us,arm54.5cost12(22.5spare),deadline187usspare.
SEEDLAT49/10/7/15/2/8us sums91us: guardstage15vsbaseline20, totalage91vs96.
This is n1 favorable measured latency, not a worst-case bound or isolated
masked-section timing. Startupraw675bus11534. SHA
4CCEB9F95E735E5812DE02800E86317D8A66A5D7514069DE1DF2CB462D5EBBDE.
Fullfixtures/provenance/finaloffPASS,portsclosed. Actual957F remainsinstalled.
Next recovery repeatability and preceding65/66 before a coordinated speedprofile
decision. No calibratedcurrent/independentrotorquality/fullgoalcompletion claim.
