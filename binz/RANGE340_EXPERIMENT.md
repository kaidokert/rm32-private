#340-profile guarded expansion — E429

Preceding evidence: EC51/335profile has3/3 original30s startup/injected-loss/
recovery completions at6.9%/~320eHz (E427–428). One7.0% hold stopped at2964us
accepted-cycle timing versus2986us floor. Current/bus and finaloff guards
passed; the excursion's cause remains unidentified. This experiment expands
the selected operating envelope, not jitter correction. All old results retain
their original pass/fail contracts; no retroactive reclassification.

The operator's progressive-expansion permission applies. Preserve the hard30%
duty ceiling,800mA PSU setting, all electrical/tracking/stop guards and finite
campaigns. This experiment initially permits only the following7.0% test step,
not a jump to30% or unlimited profile expansion.

## Coordinated profile

`bench-range340` inherits335; active values are:

- Running cycle minimum ceil(1,000,000/340)=2942us; event minimum
  floor(1,000,000/(12*340))=245us.
- Acquisition corresponding5884/490 half-us ticks; seed minimum980ticks.
- Twelve ordered intervals and seven repeated-sector checks unchanged.
  Uniform980 intervals fail5880<5884;981 can finish full qualification.
- Actual reference wait245ticks=122.5us at980. Seed age181ticks leaves64
  (32us) and passes;182 leaves63 and refuses, including across timestamp wrap.
  Observed85/87us would leave37.5/35.5us, NOT WCET or guaranteed recovery.

32us live arm floor,16us arm-cost limit, original campaign deadline,
feedback/tracking freshness,current,bus,nFAULT,IRQ and safing guards unchanged.
Any late acquisition/arm fails closed and counts as a failed attempt. This is
a narrow nominal boundary margin, not a certification of higher-speed recovery.

## Verification and next hardware plan

Candidate SHA256:
`14FEB2195EAD922D1451B630962FFF95254AE858547BF2DA4F1CA0114642A0FF`.
Release opt-s/thinLTO:text123832,data1104,bss29336. Automatic math-audit hash
matches; advisory audit is not proof of absence of expensive instructions.
357Python tests pass,340 replay191library+34other tests pass, previous335
library190 tests pass. New parser mutation test rejects mismatched profile/
acquisition metadata; edited text exists only in memory, not as bench evidence.

No flash/motor action inE429. ActualEC51 remains installed/offE428, frozen
reference/range335_ec51. Next verifyoff, flash14FE, disabledguard+fivepreflights
(CPUmax2/7/10us), then one matched61startup/69BEMF30s recovery. Require new
RUNLIMIT2942/245 and acquisition5884/490, original deadline, fresh arm>=32us/
cost<=16us, raw/bus guards, valid archives, stack>=512 and finaloff.
If that passes: ONE61startup/70BEMF10s hold; if hold passes, one30s recovery.
Stop on failure; no unchanged retry. Only a retained cohort can support a
repeatability claim. Independent rotor/current calibration/parity gaps remain.

## E430 — hardware qualification and first7.0 recovery pass

Installed14FE after verifiedoff. FirstguardUARTsilent; pre-repair snapshot
`range340_boot_snapshot01.jsonl`: APBENR108000000,CR10d/BRR22b,ISR006000c0,
ENABLEODR0/BDTR0c1a/CCRs0. Documented08040000 clockrepair restored UART.
guard02PASS3faults/18refusals; fivepreflightsPASS,CPUmax2/7/10us. Bootcause
remains unresolved. All failure artifacts retained; no motor beforepreflight.

All three powered tests PASS their requested windows, capture/timeline and
finaloff. Startup61, trace0, same safeguards; new2942/245 and5884/490 confirmed.

| Capture stem | Seconds observed | Mean eHz | COM / accepted | sigma us | IRQunion % | Raw peak / bus min mV |
| --- | ---: | ---: | --- | ---: | ---: | --- |
|range340_start61_reentry69_30s_01|27.990837 recovered|319.905544|53727 /53726|24.044768|52.709089|264 /10889|
|range340_start61_hold70_10s_01|10.000032 hold|325.762285|19546 /19545|40.935974|53.154338|299 /10901|
|range340_start61_reentry70_30s_01|27.991037 recovered|325.793157|54716 /54715|23.855346|53.128984|289 /10877|

69recovery armspare15us/cost13;70recovery seed1034,edgeage85us,remaining44.5us,
spare12.5us/cost13,acquisition6451us. Stackuntouched2676 throughout; DMA22us/
queue2. Hold sigma includes initial acceleration, not matched to recovery.
Startup rawpeaks699/439/702 remain pulse counts, not calibrated amps. No PSU
setting changed. IRQunion is not totalCPU/idle fraction or proven full throttle.

SHA256s in table order:
`B0EC9DAAAADF56FF92B986D1F39A3CCAF4C69CD96BC9158F27605379CE816C9F`;
`276A51711D9C475E2A1738C9536F51DD9F14C6CF0AB21FA0578F1EF4136A6BD7`;
`5D8E20A606B567B4246B710F7E23FB8094B53975D8BCC0D90DEDB9FFF19ED215`.

`test_drv_range340.py` passes against all three unchanged captures. This is
one7.0 recovery pass, NOT repeatability or a jitter fix; prior335-profile7.0
failure remains a failure. Next exactly two same-settings70recovery campaigns,
fail-fast, for a three-attempt cohort before considering any higher duty.
Actual14FE installed/stopped, finalgates/MOE/CCRs/ENABLE zero, UART closed.

## E431 —7.0 recovery3/3;7.1 refuses during acceleration

Same14FE/configuration, noflash.70recovery02 and03PASS original30s deadline,
one injected loss, fresh acquisition and finaloff. Declared cohort3/3 from
startup through recovery, no excluded attempts in this cohort.

02:27.990343s resumed325.767198eHz,54710COM=accepted,sigma24.003496us,
IRQ52.965060%,raw266bus10853,armspare12.5us. COMPmax54us (01/03were40),
not additive WCET or proof of preemption; no guard fault.
03:27.991275s resumed325.650064eHz,54692COM=accepted,sigma23.920673us,
IRQ52.974542%,raw299bus10853,armspare12us. Bothstack2676/finaloff/portclosed.
SHA02 `58AB49DD25F908D0CCA3419999DADEFF3A143546EAC8E3A9025B69B76E0E63EA`;
SHA03 `403717E157EC68F094C97F037715B4C228B9517E9AF9F31D2BF6A638D90FD117`.
14FE frozen `captures/reference/range340_14fe/`, hash verified before nextstep.

Then ONE declared71hold10s, startup61/same340profile, FAILED after170375us:
CycleTiming12,step4delta2930<2942us;322COM/321accepted,raw242bus11319,
stack2676. Mean315.355eHz/sigma246.521us covers acceleration, NOT a steady
315eHz result. Recent same-sector cycles~3ms; referenceprevious/refused cycles
6136/5868halfus ticks,pairedmean3001us,residualguard-reference=-4us.
No independent rotor speed or electrical/CPU fault cause established.
`range340_start61_hold71_10s_01.txt` SHA
`F94929B92ED7973E956A08709C8E9DA79AF5DB58FA5E239536F1E424386F04B6`.
Fixture exit1 retained, finalMOE/CCRs/gates/ENABLE zero and UARTclosed.
No unchanged retry.7.1unqualified;7.0highest completed cohort on this build.

Two retained-data tests in test_drv_range340 pass, preserving the failure.
Next review acceleration versus steady timing and the next profile's fresh
arm budget before any expansion. Do not interpret this170ms refusal as a
steady high-speed limit or automatically widen the guard. Actual14FE/off.
