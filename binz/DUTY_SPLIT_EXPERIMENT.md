# Independent acquisition and BEMF duty — E305

`drivedu62` still selects6.2% acquisition; `bemfdu70` selects7.0% for the
subsequent BEMF segment. Both commands only configure, never energize outputs.
`bemfdu0` inherits acquisition duty. Selection is consumed at driven_run start;
one fixed BEMF duty is passed through initial handoff and recovery. A rejected
setting does not replace an already pending setting: fixtures must explicitly
set the intended value and require its acknowledgement, never assume a stale
shell setting was cleared. Before hardware qualification audit abort-before-run
cleanup and confirm one-shot inheritance with disabled checks.

Acquisition stays40..62tenths; BEMF40..100tenths matches the current carrier
cap. Operator max300 remains permission, not implemented waveform qualification.
No speed, current, bus, actual-arm, age, storm or deadline guards changed.
The role-only PWM still forbids mid-segment duty changes. This is independent
fixed-run selection, not live throttle/ramp/governor.

Fixture --bemf-duty explicitly sends/acknowledges the request and clears it at
cleanup. DUTYSPLIT records acquisition/BEMF selection, validated against request
and acquisition capture; it is NOT an independent CCR/waveform readback.
For initial new-build tests always pass --bemf-duty, including equal-duty62.
Historical captures without the marker remain readable when no split requested.

Pure boundary/inheritance tests and host malformed/duplicate/mismatch tests
pass:181Rust,261Python,M0check. Release/s/thinLTO/codegen1 builtcandidate
09E470D5509BCD9C7509BB78D1D91C499EC3BAB6382A2822C05EDADA90E7BF9B,
text111536/data1104/bss29276. NOT flashed. Installed1E13B559,lastoffE304.
Next disabled setter/one-shot/rejection/CCR tests, then equal-duty regression and
recovery before bounded duty expansion. Do not treat compilation as qualification.

E306 adds dutycheck6cases through actual setter/consume functions and roleduN,
which reuses ENABLElow six-sector register/CCR/counter checks at requested duty.
Host drv_role_check --duty validates exact compare=floor(period*duty/1000).
Shell off now clears pending selection; generic gates_off intentionally does
not, because it is used during normal startup before selection consumption.
Rejected settings still preserve prior request. New candidate8D1AAABFCEF0D20CC9986E8C8721BBECC58C1013894E313CCC353C40CE06CCB5,
release/s/thinLTO text112316/data1104/bss29276,261Python tests pass.
No hardware actions. Next run disabled dutycheck and roledu62/70 plus preflights,
then equal-duty regression. Installed1E13B559,lastoffE304 remains unchanged.

E307 installed8D1AAABF. Initial atomic01UARTsilent, exactRCC08000000/PD1zero/
BDTRc1a/CCRs0 confirmed then knownUARTclock08040000 restore. atomic02passes256,
role62/role70 both6flags127/12us and exactCCRs, dutyselect6/6,CPU2/7/10,
archive3x3 pass. PredeclareONE10s equal acquisition62/BEMF62/phase60/trace0/
24k/backend run. Require DUTYSPLIT62/62 plus existing full fixture gates,
bus>=8400,raw<=1200,arm>=32us,cost<=16,tracking/ADC/deadline/CRC/finaloff.
No powered7% attempt in this first regression.

E307 equal62/62 regression PASS10s280.438596eHz,16827COM/16826accepted,
sigma39.225768us,IRQ58.354967%,COMP72/COM99/commit46us,arm57us/cost13,
raw271,bus11092mV,stack4180,DMA18queue2. DUTYSPLIT62/62, finaloff confirmed.
Raw f2ac13e873687e8f3ff2402028404fe6b87db74960825ef7a070c1dd3a08ded3.
Installed8D1AAABF, portsclosed. Higher-duty operation/recovery untested on this
build. Next independent increment must retain existing310profile and all guards.

E308 predeclareONE10s acquisition62/BEMF64/phase60/trace0/24k, sameinstalled
8D1AAABF. Require full fixture+DUTYSPLIT62/64, bus>=8400mV,raw<=1200,
actualarm>=32us/cost<=16us, existing310cycle/event limits, feedback/tracking/
deadline/CRC/finaloff. Stop escalation on failure; startup remains6.2%.

E30864PASS10s292.3259eHz,IRQ58.4299%,COMP72/COM97/commit46,
raw280,bus11116,arm68.5cost13,fullfixture/finaloff. Predeclare nextONE10s
acquisition62/BEMF65, otherwise identical settings/gates. Stop on failure,
especially existing cycle floor3226us; do not widen it to make this trial pass.

E30865FAILED CycleTiming12 after714760us atmean297.07397eHz. Step5 exact
guard delta714722-711501=3221us<3226;1274COM/1273accepted,raw227,bus11116,
DMA18queue2,stack4180,finaloff. DUTYSPLIT62/65 confirmed. No further increase.
CYCLECOREavg1109/previous1117/ci1137/this1113/last896/wait284/filter12;
referencecycles6837->6436halfus, pairmean3318.25us; not physicalrotorperiod.
No causal diagnosis or physicaloverspeed proof. CPUgain didnotcure thiswall.
FaultSHA25627314efe0381d3cbe8a3c7e4e959f42631c3974c1c0c7b04db9666562d8a038b.
Successful64SHA2567b036e6aa45fa8b3e483d5835e61eeccf53facf76dbdc09ea15c74d1c08ddfe6.
Both safed,portsclosed; next qualification/timing diagnosis,not guardloosening.

E309 diagnostic predeclareONE10s maximum acquisition62/BEMF65 atsame8D1AAABF,
phase60/24k, but coretrace1 for retained24handler tail. Same electrical/age/
tracking/arm/310cycle guards. A stop is retained diagnostic evidence, NOTpass;
no escalation/retries. Require full traceCRC/tail/accounting and finaloff.
Tracing perturbs persistence aperture; compare observations, not failure rate.

E309 trace65FAIL393795us,CycleTiming12 step2guarddelta3199<3226,
695COM/694accepted,raw227,bus10996,stack4172,finaloff. Tail24records:
18persistence rejects (16 onfirstread),3closedgate,2accepted,1guardrefused
after12correctlevelreads. Step2PWMcnt610/608/610 at393549/393590/393674us
recurs41/84us apart, consistent with41.656us carrier. This is retainedtail
evidence of PWM-related traffic, not whole-runcost or analogfault proof.
Finalseq6533 step2 count1033 avg1115 reads12 level0expected0 was refused
by cycleguard, not a persistencefailure. OBS and guard clocks have different
origins; do not subtract their timestamps. Referencecycles6909->6389halfus
not physicalrotorperiod. Raw03f09c8627f3c540d7d56c2b6bbe773325e15ab6e11915ba784b0b72a3a63fdb.
No further motor run; trace cleanupOFF, portsclosed. Next audit hardware
filter/routing or timed qualification opportunities preservingvalidcrossings.
