# Coordinated335 profile experiment — E426

This expands the experimental speed envelope; it does NOT fix or reclassify
the isolated cycle-timing excursions. All earlier330-profile failures remain
failures under their original contract. No335 powered result exists yet.

## Why this is the next experiment

The operator explicitly permits progressive speed-limit expansion after
preceding sensing/timing/current/bus/tracking validation. E421 and E425 each
completed the30s recovery campaign near315–316eHz at6.8% duty, with near1:1
COM/accepted events, sigma about23.5us, finaloff and ample raw-current/bus
guard margin. Rawcounts are not calibrated amps; independent rotor timing and
broader repeatability remain incomplete. E421's6.9% run stopped at3017us,
slightly outside the3031us experimental boundary, not a demonstrated silicon
ceiling. We must not demand a proven latency improvement before EVERY profile
experiment: that additional prerequisite was not the operator's requirement.

E425 found no carrier-preparation gain. Candidate1717 and its audit are saved
in `captures/reference/earlycarrier_1717/`; the next build excludes that option.

## Coordinated limits and invariants

Opt-in `bench-range335` inherits330 but takes precedence in active constants.
Running full cycle minimum isceil(1,000,000/335)=2986us; inter-event minimum
isfloor(1,000,000/(12*335))=248us. Acquisition uses corresponding5972/496
half-us tick floors. Seed minimum isfloor(2,000,000/(6*335))=995 ticks.
Twelve ordered intervals and seven repeated-sector cycle checks remain.
Uniform995 intervals still fail the cycle floor (5970<5972);996 can qualify.

The actual reference wait at995 is249 ticks. Handoff at185 ticks age leaves
exactly64 ticks (32us) and passes;186 fails, including across timestamp wrap.
There is no grace period, timestamp refresh, threshold averaging, or fallback
that grants output authority to an expired seed. The existing16us arm-cost
limit,1ms feedback/tracking deadlines, current/bus/nFAULT/IRQ guards and original
campaign deadline remain unchanged. Failed acquisition/arm remains a retained
failed attempt, not an excuse for an automatic retry.

A measured worst-case latency bound remains useful for reliability claims,
but its absence does not require indefinite refusal to conduct a guarded
experiment that safely rejects late seeds. Passing335 would qualify only the
tested settings; it would not establish general recovery WCET.

## Staged verification and bench plan

Release opt-s/thin-LTO candidate:
`EC510EAC16A5DFDD20713A3FAA7AE0726E346DC3A5D78524920D2CE88A3BC460`.
text123836/data1104/bss29336. Automatic hashed math audit matches the ELF;
advisory findings are not a proof of absence of expensive arithmetic.
354Python tests pass;335 replay190library+34other tests pass; existing330
library189 tests also pass. A fixed1000-tick driven-seed test was correctly
updated to use an interval below the selected cycle floor:1000 is now valid
for335 and cannot be used as evidence of an excessive-speed fault.
Synthetic parser tests require matched runtime/acquisition metadata; they
modify text in memory only and are not presented as hardware results.

Next: verify off, flash candidate, disabled guard and five preflights. Require
CPU2/7/10us ceilings and finaloff. First powered test: startup6.1%, BEMF6.8%,
30s one injected tracking-loss recovery, all original fixture gates, explicit
RUNLIMIT2986/248 and RECOVERYCYCLE5972/496. If it passes, ONE6.9% duty campaign
is justified by the separately declared wider profile, not carrier relocation.
Require original deadline completion, fresh seed arm>=32us/cost<=16us,
valid archives, no tracking/electrical/refusal fault, stack>=512 untouched and
outputs off. Any failed attempt is retained and diagnosed before another test.
No larger duty or further profile is approved by this experiment itself.

No hardware action in E426. Actual1717 remains installed/stopped fromE425;
rootEC51 is only the next candidate.

## E427 — installed; first6.8 and6.9 recovery campaigns pass

Preflash UART verifiedoff. InstalledEC51; guard01UARTsilent. Pre-repair
`range335_boot_snapshot01.jsonl` confirms APBENR108000000, CR10d/BRR22b,
ISR00600010, ENABLEODR0/BDTR0c1a/CCRs0. Documented clock repair08040000
restored UART; guard02PASS3faults/18refusals. All five
`range335_preflight01` checks passed, CPUmax2/7/10us. Boot defect unresolved.

Both motor attempts used startup61, original30s campaign, injected loss at2s,
one recovery, trace0 and all prior fixture/provenance checks. New live profile
2986/248 and acquisition5972/496 confirmed. No earlycarrier feature.

| Capture suffix | BEMF duty | Recovered seconds | Mean eHz | COM / accepted | Cycle sigma us | Raw peak / bus min mV | Arm spare us |
| --- | ---: | ---: | ---: | --- | ---: | --- | ---: |
| range335_start61_reentry68_30s_01 | 6.8% | 27.991039 | 315.702989 | 53021 / 53021 | 23.691431 | 272 / 11032 | 17.5 |
| range335_start61_reentry69_30s_01 | 6.9% | 27.990912 | 319.831632 | 53715 / 53714 | 24.251296 | 288 / 10829 | 15.5 |

Both PASS original deadline, archives/timeline, stack2676 untouched and final
MOE/CCRs/gates/ENABLE zero, UART closed. DMAmax22us/queue2. IRQunion52.506353%
and52.866458%, respectively; not totalCPU or idle fraction. Arm costs13/12us.
Startup rawpeaks997/424 (<1200 backstop), not calibrated amps. No PSU setting
changed; hardware current-limit setting remains operator-provided.

Capture SHA256s:
-68: `15FB69E0087FF25AB707FB756508D9D21BAF574E5A20151F12DCAE222764735C`.
-69: `26B62A434858F82A044AEB55D6320303084028C273EF75715AED508275310AB3`.

This is the FIRST6.9 recovery pass in this profile, not a completed reliability
cohort. Do not infer disappearance of outliers or retroactively pass old runs.
Next a fixed two additional same-settings6.9 campaigns, fail-fast, to establish
a3-attempt cohort before contemplating more duty. ActualEC51 remains stopped.

## E428 —3/3 at6.9%;7.0% encounters the next boundary

Additional69 campaigns02 and03 both PASS original30s deadline/fresh recovery.
Declared EC51/335/61startup/69BEMF cohort denominator3/3, no omitted attempts.
02:27.990641s resumed319.822172eHz,53712COM=accepted,sigma24.190044us,
IRQ52.730020%,raw269bus10889,armspare14.5us.
03:27.990917s resumed319.866014eHz,53720COM=accepted,sigma23.905022us,
IRQ52.724309%,raw291bus10913,armspare15.5us. Both stack2676/finaloff.
SHA25602: `E77AF8C90CF458EF383237EB51DF60A7F5776142DA27E9E0AA17DC6C0A0CBFC8`.
SHA25603: `F81423E0E567F27780E4A5274F490A0C27ED8F8E6A8D6754F4B7DA75ADCAD857`.
EC51 binary frozen `captures/reference/range335_ec51/`, hash verified.

Then ONE declared10s hold at7.0%,same profile/guards:
`range335_start61_hold70_10s_01.txt` FAILED after3.702165s at325.193374eHz.
7223COM/7222accepted,raw274,bus10925,stack2676,DMA22us/queue2. CycleTiming12:
step2 previous3699168->3702132=2964us below2986. Reference6332/5921 half-us
ticks, paired mean3063.25us; guard-reference residual3.5us. Recorder previous
same-sector stamp24us after its guard. This remains accepted-event timing,
not independent rotor speed, ISR latency attribution or hardware fault proof.
Overall sigma61.450us includes initial acceleration, unlike recovered windows.
SHA256 `C6B0DAFDFD9FD1F4F5C3436995ECA45F246C12AAFBA10680678E94F03F8535A2`.
Fixture exit1 retained, finaloff verified, port closed. No unchanged retry or
higher duty. `test_drv_range335.py`2 tests pass against the four raw captures,
explicitly preserving7.0's failure. ActualEC51 remains stopped.

Next review7.0's boundary and fresh-arm budget together before further profile
experiment. Do not relabel failure or claim current/PSU/CPU saturation caused it.
