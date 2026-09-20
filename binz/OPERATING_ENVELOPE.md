# Current measured operating envelope

E450ACEE inlinecandidate72cohort2PASS/1FAIL, thirdfailed24.758s CycleTiming
2842<2858,finaloff. Age85/arm7 repeats allthree; armgain notsustainedfix.
No higherduty qualified. Earlier referencecohorts retain original outcomes.

E445 candidate7C37 at72:1PASS/1FAIL, thirdcancelled. Failure24.287s recovered,
CycleTiming2854<2858,finaloff. Arm8us repeated but sustainedqualification did
not. Earlier7F4B72cohort3/3 remains reference, notrare-failure guarantee.

E440 unchanged7F4B:7.3% hold FAILED197ms CycleTiming2844<2858,finaloff.
Tailmedian2917us/~342.82eHz vs aggregate327includingacceleration; neither
steady-state nor physicaloverspeed proved. No retry/recovery.7.2cohort remains
highest3/3; failure mechanism beyond selected cycle boundary unidentified.

E439 unchanged7F4B/350:7.2% original30s recovery cohort3/3 at336.40..336.54eHz,
sigma23.75..23.77us,IRQ53.02..53.20%,armspare>=7.5us,stack2676,allfinaloff.
No guards widened; raw current counts still not amperes. Higherduty untested.

E438 unchanged7F4B/350:7.2% hold10s and firstrecovery30sPASS; recovery336.451eHz,
sigma23.754us,IRQ53.195%,armspare7.5us,raw310bus10805,stack2676,finaloff.
One recoverypass, notcohort. Next fixedtwo72 repeats; no guards widened.

E4377F4B/350:7.1% original30s startup/recovery cohort now3/3,332.20..332.23eHz,
sigma22.89..22.97us, IRQ52.93..53.15%, minimumarmspare9us,stack2676.
Allfinaloff; frozenreference/range350_7f4b. Earlier narrower-profile failures
remain failures. Current raw counts are not amperes; no jitter fix established.

E4367F4B/350: first71recovery30sPASS~332.201eHz,55793COM/55792accepted,
sigma22.968us,IRQ52.961%,raw312bus10901,armspare9us,stack2676,finaloff.
Onepass only; nexttwo71repeatability runs. Preceding70recovery alsoPASS.
OldFAA0/34571failure remainsfailed; no jitterfix claimed.

E434 actualFAA0/345:71hold10sPASS~332eHz,but71recoveryFAILED8.245s resumed
~333eHz,CycleTiming2881<2899. Seed/arm succeeded (9.5usspare); not an entry
failure. Allfinaloff. Highest3/3cohort remains14FE/340/70, not71/newFAA0.

E43114FE/340:7.0% recoverycohort3/3 original30s,325.65..325.79eHz,
armspare>=12us,stack2676,allfinaloff;buildfrozenreference/range340_14fe.
Next7.1% holdFAILED170ms duringacceleration:cycle2930<2942us,raw242bus11319.
Not a steady315eHz limit (reportedmean includesacceleration).7.1unqualified.

E430 actual14FE/340profile:70BEMF hold10sPASS and first30s injected-loss recovery
PASS325.793eHz,54716COM/54715accepted,sigma23.855us,IRQ53.129%,raw289bus10877,
armspare12.5us,stack2676,finaloff. Onepass only; nexttwo70repeatability runs.
Preceding69recovery alsoPASS. New2942/245profile, notjitterfix; old33570failure
remains failed. RANGE340_EXPERIMENT.md hashes/currentmeasurementlimitations.

E428 EC51/335profile:61startup/69BEMF30s injected-loss recovery cohort3/3,
319.82..319.87eHz,sigma23.91..24.25us,IRQ52.72..52.87%,stack2676,allfinaloff.
Build frozen reference/range335_ec51. NextONE70hold10s FAILED3.702165s,
325.193eHz,acceptedcycle2964<2986us,raw274bus10925,finaloff.7.0unqualified;
selectedprofileboundary isnot independentrotor/CPU/hardwareproof.

E427 actualEC51 range335: first61startup/69BEMF30s recovery PASS27.990912s
resumed319.832eHz,53715COM/53714accepted,sigma24.251us,IRQ52.866%,raw288,
bus10829,armspare15.5us,stack2676,finaloff. Preceding68recovery alsoPASS.
New2986/248us profile, not jitterfix; prior330failures remain failures.
One69pass is not repeatability. Next fixedtwo69campaigns fail-fast, no higher
duty yet. Full hashes/gates in RANGE335_EXPERIMENT.md.

E421 installed137F quiet IRQstamp:61startup/68BEMF original30s recoveryPASS,
315.395eHz,IRQ52.738%,sigma23.731us. One69recoveryFAILED after27.1338s resumed
319.662eHz,CycleTiming3017<3031us. Bothfinaloff.6.9 remainsunqualified.

E415 restored archivedA2DE DMApeer baseline; decision/criticalexperiments off.
Startup61/BEMF68 original30s recovery PASS27.990602s resumed315.7165eHz,
53023COM/53022accepted,IRQ53.530%,sigma24.119us,raw260bus10901,stack2676.
Outputs off verified. DiagnosticF730 archived, notinstalled (rootELF stillF730).
This restores preceding operating point, not new6.9qualification.

E414 currentF730 decision diagnostic: startup61/BEMF68 hold10s passed, but
original30s recovery FAILED2.666888s after successful reentry at316.347eHz.
CycleTiming3015<3031us;IRQ70.405%,raw319bus11056,stack2120,finaloff.
No recovery qualification on this diagnostic. Gate-to-accept6.5us at fault,
same as prior accepted event; no long qualification stall observed there.
Plan to retire costly diagnostic before further envelope work; stillinstalled.

E404 installed diagnostic044F adds bounded reference COMP-service exclusion.
6.8%10s hold and original30s recovery passed; recovered315.863eHz,
53047COM/53047accepted,IRQ59.034%,sigma28.461us,raw254bus10996.
One6.9% attempt FAILED at0.160s BEFORE dropout: CycleTiming3029<3031us.
Critical-bodymax48us/refused0,finaloff verified.6.9% remains unqualified;
do not infer improved reliability from the preceding6.8% pass.

G071 + DRV8304H, existing free-wire map, 24,006Hz BEMF PWM. Startup stays
at6.2%; table duty is the independently commanded BEMF duty, not RPM percent.
Supply setting remains operator-reported11.7V/800mA; there is no fresh matched
PSU-current reading. Raw current protections are active but not calibrated amps.

## Installed9789F836 exact ADC/timeline bins, E392

Superseded as installed by E400 A2DE7D98 DMApeer (COMP/COM/DMA64, guard0).
6.8%10s hold and original30s recovery PASS315.365/315.568eHz. Recovery
52999COM/52998accepted,IRQ53.135%,sigma24.417us,raw285bus10972,stack2676,
COMP41/COM48/commit21/record19/DMA22us,queue2,17usarmspare,finaloff.
Only DMApriority change plus idleprobehooks; same3031/252 profile/guards.
Next6.9recovery; not qualified at6.9yet. SeeDMA_PEER_EXPERIMENT.md.

E401: that6.9% recovery FAILED after13.088722s resumed at320.007eHz,
CycleTiming12 step3 delta3011<3031us.25131COM/25130accepted,raw263bus10937,
IRQ53.125%,sigma24.428us,stack2676,finaloff. DMApeer improves measured wall
timing but does not eliminate the boundary fault.6.9% remains unqualified.

E393 supersedes the next-action below: attempted original30s6.9% recovery
FAILED after successful fresh acquisition/re-arm and5.005492s resumed drive.
320.143742eHz,9615COM/9614accepted,cycle sigma26.062005us,IRQ53.426692%.
CycleTiming12: step4 same-sector3012us below3031us floor by19us. Raw256counts,
bus11104mV,stack2696,finaloffverified. Seed1059ticks,actualarm47.5us/cost12us.
This is NOT a completed recovery despite result7 (path entered). Capture
binmath_reentry69_30s_01.txt retained; no higher duty or guard changes.
Reduced arithmetic overhead has not eliminated the intermittent boundary fault.

Same330profile/priorities/guards. One original30s6.8% recovery PASS315.824eHz,
53041COM/53040accepted,IRQ53.247%,sigma25.969us,raw278bus11092mV,
COMP59/COM63/commit37us,recorder35/DMA14us,stack2696. Recovery85us seedage,
17usarmspare. Then6.9% hold PASS10s319.911eHz19195COM/19194accepted,
IRQ53.233%,raw264bus10901,COMP59/COM61/commit37us. Finaloffverifiedboth.
Onehold at6.9%, not recovery/repeatabilityqualification; E383failure remains
historical and its specificcause notproven. Next6.9% recovery beforehigherduty.
Startup6.9% capture raw977/1200guard distinct from running264counts.

## ArchivedCC71BC84 prepare-only carrier compare, E389

Same330profile, one original30s6.8% recovery PASS315.805eHz,53038COM/accepted,
IRQ54.724%,sigma27.804us,raw284counts,bus10937mV,stack2696. ObservedCOMP65/
COM67/commit41us; recoveryseedage85us,arm49us(17spare). Finaloff verified.
LowerCOM/commit maxima than A727's69/45us, not a WCET or exclusive cost claim.
No6.9% attempt or widerprofile; currentcalibration/independentquality stillopen.

## ArchivedA727F8D7 recovery clear-once, E386

Same330profile, one original30s6.8% recovery PASS316.398eHz,53138COM/
53137accepted, IRQ55.209%, sigma28.268us. Raw247counts,bus10937mV,
COMP65/COM69/commit45us,stack2696. Seed-to-arm85us versus87us onF715,
actualarm47.5us with15.5us spare,deadline215us spare. Finaloff verified.
N1 candidate qualification, not repeatabilitycohort/WCET or a higher point.
F715's6.9% refusal remains evidence; no6.9% attempt on this candidate.

## ArchivedF7155BFB330 profile, E381

Runtime3031/252us and acquisition6062/504half-us bounds. All independent
electrical/age/arm/deadline guards retained. Matched6.6% hold and original30s
recovery PASS near304eHz, recoveryarmspare20us. Then6.7% hold PASS10s near
309.987eHz, IRQ55.545%, raw265counts,bus10913mV, COMP65/COM69/commit45us.
E382 adds original30s6.7% recovery PASS310.116eHz,17usarmspare, and6.8% hold
PASS316.515eHz/recoveryPASS316.583eHz,14usarmspare. Recovery68IRQ55.334%,
sigma28.315us,raw248bus10984,COMP65/COM70/commit45us,stack2696. One pair
perpoint, notrepeatabilitycohorts.
E383: same-profile6.9% hold stopped after421292us with CycleTiming12:
step1 guard interval3009us <3031us.797COM/796accepted, raw247counts,
bus11044mV, outputs-off verified. Reference cycles6424/6012half-us ticks
have paired mean3109us; these are controller counters, not independent rotor
periods. Short-window mean315.320eHz is not equilibrium speed. No recovery
or higher duty attempted; no automatic profile increase. Physical overspeed
versus acceptance-timing variation remains unresolved.
Allfinaloff verified. Newprofile
does not retroactively pass the old6.7% refusals or establish jitter reduction.

## Archived4D43E26B lean exact seed math, E379

At6.4%, one original30s recovery PASS near292.845eHz (E378). E379 adds one
10s hold and one original30s recovery at each following point, all finaloff:

| BEMF duty | Hold | Recovery | Recovered eHz | IRQ union | Recovery arm spare |
| --- | --- | --- | ---: | ---: | ---: |
| 6.5% | PASS | PASS | 299.879 | 54.880% | 22us |
| 6.6% | PASS | PASS | 304.320 | 55.576% | 20.5us |

Both recoverySEEDLAT87us, COMP65/COM69/commit45us, stack2696, raw276/250,
bus11008/10769mV. Onepair perpoint, not repeatabilitycohorts. No widerprofile
enabled yet; earlier6.7% refusals remain archived evidence, not erased.

## Archived957F6C5A guard installation, E368

Same320profile and all guards. At6.4%, fixed3/3 original30s injected-loss
recovery attempts PASS; raw hashes in captures/guardinstall_reentry64_30s_cohort.csv.
Recovered292.728..292.870eHz, IRQ54.769..55.283%, cycle sigma24.556..25.076us.
Recovery arm spare20.5..22.5us, guard-start stage15us and total seed age91us
in all three. COMP65/COM69/commit45us observed maxima; raw267..290counts,
bus10817..10937mV, stack untouched2696bytes. Every finaloff verified.
One10s hold also passed at6.4% (E367).

E369 adds one hold and one recovery each at6.5/6.6%, not repeat cohorts:

| BEMF duty | Hold | Original30s recovery | Recovered eHz | IRQ union | Recovery arm spare |
| --- | --- | --- | ---: | ---: | ---: |
| 6.5% | PASS10s | PASS | 299.765 | 55.283% | 18us |
| 6.6% | PASS10s | PASS | 304.363 | 55.490% | 16us |

Both recovery runs: COMP65/COM69/commit45us, guard-start15us, total seedage91us,
armcost12us, untouchedstack2696. Raw running peaks278/247counts,bus10984/10769mV.
The6.6% hold startup reached974/1200rawcounts; running peak250 is distinct.
All four fullfixture/provenance/finaloff checks passed. No speedprofile change
yet; old6.7% failure below is on58B570 and is not erased by these passes.

## Archived58B570 static comparator, E360

Same320profile and allguards. At6.4%,3/3 original30s recovery cohort passes
292.772..292.969eHz,IRQ54.818..55.199%; hashes in
captures/staticcomp_reentry64_30s_cohort.csv. Higher rows are one hold and one
recovery each, not repeatability cohorts:

| BEMF duty | Hold | Original30s recovery | Recovered eHz | IRQ union | Recovery arm spare |
| --- | --- | --- | ---: | ---: | ---: |
| 6.5% | PASS10s | PASS | 299.907 | 54.911% | 12.5us |
| 6.6% | PASS10s | PASS | 304.197 | 55.512% | 11us |

Recovered6.6% maximumCOMP78us (not earlier65),COM69/commit45us;raw270,
bus10865mV. Finaloff verified after every attempt.
E3616.7% candidate FAILCycleTiming12 after304442us: step4 interval3124us
violates3125floor.555COM/554accepted,raw212bus11140,COMP69COM68commit45.
Shortfailed-windowIRQ54.769% is not a qualified operating point. Physical
overspeed/crossing-variation cause remains unproven. No higherduty attempted.

## Archived build28DAC42D, explicit320 test profile

Cycle/event minima3125/260us; actual seed arm minimum32us; other electrical,
tracking, feedback-age and finite-deadline guards unchanged. Each row is one
hold and one injected-loss/recovery attempt, not a repeatability cohort.

| BEMF duty | Hold | Original30s recovery | Recovered eHz | IRQ union | Recovery arm spare |
| --- | --- | --- | ---: | ---: | ---: |
| 6.4% | 10s pass | pass | 292.79 | 58.66% | 16us |
| 6.5% | 10s pass | pass | 299.97 | 58.76% | 12.5us |
| 6.6% | 10s pass | pass | 304.20 | 58.83% | 11us |
| 6.7% | stopped at191ms | not attempted | not established | not a pass | unknown |

Raw captures: `captures/range320_hold{64,65,66}_01.txt` and
`captures/range320_reentry{64,65,66}_30s_01.txt`. Every completed attempt above
passes full chronology/CRC/electrical/recovery/deadline and finaloutputs-off
verification. RecoveredCOM:accepted counts agree to within one event; this
is scheduling agreement, not an independent rotor-crossing quality denominator.

Allthree recovery segments measured COMmax69us, COMP71us, commit45us. Those
are observed wall brackets, not worst-case proofs. IRQ union excludes foreground
work; the remaining fraction is NOT idle. Neither duty nor speed implies a
linear CPU-load extrapolation.

The6.7% attempt (`captures/range320_hold67_01.txt`) stopped at an actual3111us
same-phase interval against3125us minimum. It was still in the early run;
whole-window mean299.4eHz is not its equilibrium speed. The last paired
reference cycles averaged3218.5us, not an independent rotor measurement.
COMmax67us and raw215/bus11343mV do not indicate an electrical or COM-latency
trip. No recovery or higher duty was attempted after this failure.

## Earlier profile and retained boundary failure

ArchivedBA1F982B used cycle/event minima3226/268us. At6.4%, its fixed3/3
original30s recovery cohort passed near292.4eHz (manifest:
`captures/peerexti_reentry64_30s_cohort.csv`). At6.5%,
`captures/peerexti_hold65_01.txt` stopped after3.34s: measured same-phase
interval3211us violated3226us. This remains a failed run under that profile.
The later wider profile did not retroactively fix it or prove reduced jitter.

## Limits of the evidence

- No hardware-qualified320eHz point yet;320 is a guard-profile name.
- Startup can approach its raw-current guard (1147/1200 on the earlier6.4%
  cohort); low sustained current cannot establish startup margin.
- Exclusive CPU attribution remains unmeasured: both detailed and root-bucket
  diagnostic meters exceeded the unchanged10us disabled overhead gate.
- Current calibration, independent BEMF-quality comparison, higher-point
  repeatability and complete archive-grounded parity remain open.
- The objective's30% duty maximum is permission, not a duty target to force.
