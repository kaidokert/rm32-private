# Static live-comparator adapter experiment — E357

E358 installed candidate; first10s hold and30s recovery PASS, finaloff verified.
Known baseline28DAC42D remains archived. Wider candidate qualification is open.
Candidate58B570E06486AE16FB868AF83AFDEFCBED8A8BB270214623266E95C21E1CCA0B.

Baseline release assembly comp_isr at080040e0 retains source and trace-mode
branches inside persistence (0800415e and08004174), despite caching flags once.
This is not an exclusive CPU cost measurement. Existing cache is not missing.

Optional bench-static-comp implies inline/cached-comp. In trace-off COMP
dispatch, construct the normal per-invocation mode snapshot. Only real AND
inverted AND trace-off selects StaticComp. Its read uses the existing helper
with constant true/true/false, still reading volatile COMP2 CSR each time.
Other comparator methods delegate to original Comp, retaining live masking,
pending, ownership and enable checks. Other modes retain normal motor adapter.
Shared AM32 source, persistence count, interval gate and acceptance unchanged.
No ADC, current, bus, tracking, rate, arm or deadline changes.

Build release/s/thinLTO/codegen1 PASS. Text121756 vs archived121264 (+492),
data1104/bss29324 unchanged. Candidate also includes source changes since archive;
do not attribute every byte or later timing difference solely to specialization.
301Python tests pass including strict COMPSTATIC provenance and legacy handling.
Fixture flag --static-comp requires the marker; marker states compiled capability,
not a count proving every dispatch selected the specialization.

Important: faster consecutive reads shorten physical persistence aperture. The
same sample count is not identical analog filtering. Must qualify behavior, not
declare the optimization free from its source semantics.

Before power: review specialized generated code and invocation-mode invariants,
then disabled priority/pulse/atomic/roles/CPU/archive checks. First matched10s
hold startup62/BEMF64,phase60,trace0,24006Hz with --static-comp. Require complete
original fixture guards, actualarm>=32us, armcost<=16us, current deviation<=1200,
bus>=8400mV and finaloff. Compare E355 speed/IRQ58.774%/sigma38.048us/maxima
COMP71/COM69/commit45us; no prescribed gain. Recovery qualification must precede
higher-duty exploration. Retain failure, never tune guards to rescue this build.

## E359 repeatability and next duty point

Fixed64recovery cohort now3/3PASS; see captures/staticcomp_reentry64_30s_cohort.csv
with verified raw hashes. Recovered292.772..292.969eHz, IRQ54.818..55.199%,
sigma24.799..25.108us,COMP65COM69commit45,stack2696,armspare17..18us.
All startup attempts and original-budget recoveries passed; raw running peaks
271..287counts,bus10889..11116mV. Startup raw556/510/719 is distinct.

staticcomp_hold65_01 subsequentlyPASS10s299.737697eHz17985COM/17984accepted,
IRQ54.858436%,sigma39.685148us,COMP65COM69commit45,raw232,bus10793mV,
entryarm67.5us/cost13,stack2696; startupraw501bus11534.
SHA2C44F04772301451AF347B54D9150AA0B86E19A8B845043ED59917699D7EB33D.
Finaloff verified every attempt. No currentcalibration or higherpoint recovery
claim. Next65recovery before66; no further equivalent64cohort needed.

## E358 initial hardware evidence

Generated ADC_COMP specialized loop08001c88..08001cb0 reads CSR at08001c90
each iteration, with no source/trace selection inside that loop. Shared routine
fallback remains. Initial priority UART failure retained; exact disabled
RCC/PD1/BDTR/CCR checks preceded clock repair. priority02 three trialsPASS,
preflight01 all fivePASS, CPUmax2/7/10us.

staticcomp_hold64_01 PASS10s292.764eHz17566COM/17565accepted,IRQ54.781862%,
sigma34.335750us,COMP65/COM69/commit45us,raw279,bus11151mV,entryarm68us/
cost13,stack3356. SHA A818E333CDFE294546A73D6277047C839AF33D6ADC112F869BB824CC38457A53.
E355 same-setting baseline292.576eHz/IRQ58.774%: about4percentage points lower,
despite19566vs18347 COMPvisits/s. n1 comparison, not exclusive cost proof.

staticcomp_reentry64_30s_01 PASS original30s injected-loss/fresh-seed recovery:
recovered27.989943s292.772eHz49168COM/49168accepted,IRQ55.198629%,sigma24.799390us,
COMP65/COM69/commit45us,raw271,bus11116mV,stack2696. Recoveryseed1160ticks,
acquisition7474us,actualarm49us (17us above unchanged32usfloor), originaldeadline
157us spare. SHA792F5836E9FFD8F8C48F3191A547E48AC4AE0DA9487371FAB2F6B47123FD1F9B.
Both full fixtures/provenance/finaloff pass, ports closed. No independent rotor
quality or calibrated current claim. Next repeatability/recovery qualification
before rechecking the preceding6.5/6.6 envelope and6.7 refusal boundary.
