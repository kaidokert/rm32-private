# Guard constructor code generation — E363

E364 tested then PARKED: recovery arm age worsened. Restored qualified58B570,
disabled priority/finaloff verified. Archive:
captures/reference/staticcomp_58b5/shell-pwm.elf.
Finalcandidate7A6E96AA0382BB0AD8BE69D5D1DBAE4AD67604402CCBA61687D29F9C8B9258B9.

Recovery SEEDLAT identified a20us wall bracket around guard startup. Inspection
shows profile validation, fresh feedback/age/deadline/step checks, construction,
installation and TIM6 start. Statistics ALREADY stage before seed acquisition.
Most admission inputs are live; don't blindly preconstruct an authorized guard.

Smaller experiment: feature bench-inline-guard adds only inline(always) to
RunGuard::with_limits. No algorithm/state/ownership change, new unsafe code or
timing-policy change. Source checks remain in original order. Host replay feature
enables the same attribute;203Rust and305Python tests pass. Fixture --inline-guard
strictly requires GUARDCODE inline_constructor=1 admission_unchanged=1 prestaged=0.

Initial candidateB92C (before marker) start_inner localstack180->132bytes;
with_limits symbol/call disappears. Memory clear/copy work remains: don't claim
zero-copy or20us saved. Finalcandidate text121668 vsbaseline121756 (-88bytes),
data1104/bss29324 unchanged. Release/s/thinLTO/codegen1 buildPASS.

Next: finalassembly check, disabledpriority/pulse/atomic/roles/CPU/archive, then
10s64hold and30s64recovery with --static-comp --inline-guard. Preserve current,
bus, ADCage, eventage,32usactualarmfloor,16usarmcost and original deadlines.
Compare validated SEEDLAT brackets and actualarmage to E358/359, not justcode
size. Require completefixture/finaloff, retainfailure. No67repeat orspeedprofile
increase before a demonstrated gain and preceding-point qualification.

## E364 result: smaller is not faster here

Finalassembly confirms132byte localstack, live validation and memorycopy remain.
Installed7A6E, priority01threePASS/preflight01all5PASS CPU2/7/10; no UARTrepair.
inlineguard_hold64_01 PASS10s292.630eHz/IRQ54.727%,raw261bus10984,
COMP65COM69commit45,entryarm55.5cost12,stack3356; startupraw997.
SHA092DF0FA45AFD5D0074BC772646BA5E0A98741196BE173EBAC46BF65ECEB9715.

inlineguard_reentry64_30s_01 PASSoriginal30s292.835859eHz49180COM/49179acc,
sigma24.801852us,COMP65COM69commit45,raw264bus10889,stack2696.
Seed1153/acq7428us,arm42us/cost12 (10usspare),deadline250usspare.
SEEDLAT brackets48/10/8/26/2/8us: guardstartup26vsbaseline20, totalarmage
102vs96us. This n1 result does NOT establish a favorable latency change;
smaller localframe andtext were insufficient. No higherduty/cohort attempted.
SHA69273151CEFE6196710BD701F536A5C3EAB31EE4A99FC53EEC37AF71D2F8CD08.
Both fullfixtures/provenance/finaloffPASS,portsclosed. Restored archived58B570;
inlineguard_restore_priority01threePASS/finaloff,noUARTrepair. Root7A6E ELF
is NOTinstalled. Optionalfeature remains parked, not a qualified portablewin.

## E365: borrowed Option rejected offline

Temporary localSome(guard)/take() experiment preserved admission/publication
order but generated TWO68bytecopies inside maskedclosure at08003e74/08003e84,
versusbaselineonecopy. Callerlocalstack180->188,closureadds76bytes,text+80.
BuildAF4226C57A84C7C8202F6D799AC6C9739476C6B8C213BDE843D7594F26791A75
neverflashed; feature/source removed. Rootbinary remains rejected; don'tflash.
Actual58B570,lastoffE364 unchanged. Next separate freshvalidation from inplace
installation while retaining refusalorder/age/deadlines/atomicpublication.
No artificialhealthy guard may authorize outputs before freshvalidation.
