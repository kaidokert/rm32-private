#345-profile expansion — E433

Rationale: preceding14FE/340/7.0% cohort is3/3 original30s startup/recovery
around326eHz. E431's7.1% acceleration failure had a local tail median332eHz,
not its315eHz startup-diluted aggregate; guard2930us exceeded the selected
340profile boundary. This is explicit envelope expansion, NOT a jitter fix.
Old failures retain their original contracts. Calibration and independent
rotor timing remain incomplete; no hardware/CPU limit claim is made.

Opt-in bench-range345 inherits340 with priority in active constants:
runningcycle ceil(1e6/345)=2899us,event floor(1e6/(12*345))=241us;
acquisition5798/482 half-us ticks, minimum seed966ticks. Twelve ordered
intervals/seven cycle checks unchanged. Uniform966 gaps fail5796<5798;
967 gaps can complete qualification. No output authority from the profile.

Reference wait242ticks=121us at966seed. Age178ticks(89us) leaves64ticks and
passes;179ticks refuses, including timestamp wrap. Observed85/87us would leave
4/2us spare above32us. Narrow nominal headroom is NOT WCET. The live32us floor,
16us arm-cost limit,current/bus/nFAULT/feedback/tracking/IRQ guards, original
deadline and one-recovery cap remain unchanged. Late seeds safely refuse and
count as failures; never refresh timestamps or retry around refusal.

Candidate SHA256
`FAA0FE858FE21A8456A647FEF36C3818A0A12D7A15B14F94FC0516033FF017B4`.
Release opt-s/thinLTO:text123836,data1104,bss29336. Hashed automatic math-audit
matches ELF (advisory, not division-free proof).362Python testsPASS;
345 replay192library+34other testsPASS. Previous340library191 testsPASS.
Parser mutation tests require matched profile/acquisition metadata without
creating synthetic hardware captures. New boundary tests cover full sequence,
cycle floor, minimum seed and age refusal across wrap.

No hardware action inE433. Actual14FE/offE431 remains installed and archived.
Next verifyoff, flashFAA0, disabledguard+fivepreflights (CPUmax2/7/10us).
Then startup61/BEMF70 original30s one injected-loss recovery; require live
RUNLIMIT2899/241 and recoverycycle5798/482, valid fresharm>=32us/cost<=16us,
originaldeadline, electrical/tracking checks, archives, stack>=512 and finaloff.
Only afterpass: ONE71hold10s; ifpass one71recovery30s. Fail-fast, no unchanged
retry or higherduty. This tests expansion, not a claim of335/340fault repair.

## E434 —7.1 hold passes; resumed sustained window fails

InstalledFAA0 afteroff. Bootguard01UARTsilent; pre-repair snapshot APBENR1
08000000,USARTCR10d/BRR22b/ISR00600010,PD1ODR0/BDTR0c1a/CCRs0. Documented
08040000 clockrepair restoredUART. guard02PASS3faults18refusals;fivepreflights
PASS,CPU2/7/10us. Bootcause unresolved; allcaptures retained.

range345_start61_reentry70_30s_01 PASS original30s,27.991239s resumed,
325.584521eHz,54681COM=accepted,sigma24.112330us,IRQ52.913590%,raw281bus10853,
seed1045/age85us/armspare13.5/cost13,stack2676.
SHA `E0D596DDA07D4F684E90699DEEFD0A451889F09E2F73695EFCE257B6B92E07D0`.

range345_start61_hold71_10s_01 PASS10.000003s,331.994834eHz,19920COM/19919acc,
sigma41.960222us includingstartup,IRQ53.252192%,raw294bus10889,stack2676.
SHA `D3D1CFC8C9D176A6E8B734099ED2FA72CFF9AF3A12F93081C2367865F6B37366`.

range345_start61_reentry71_30s_01 FAILED after8.245294s resumed332.571966eHz,
16453COM/16452accepted,sigma23.383271us,raw300bus11032,stack2676.
Reacquisition succeeded(seed1013/acq6677us); age85us,remaining41.5us,
armspare9.5us,cost13. Thus THIS failure is sustained-cycle timing, not expired
seed or failedreentry. CycleTiming12 step2delta2881<2899us. Referencecycles
6220/5758ticks,pairedmean2994.5us;guard-reference residual2us. DMAmax24/queue2.
No physicaloverspeed or ISR/analogcause proven. SHA
`4EF52F6AAFACF43901CB75C25A6316390EC4FBFA4E36B89FCF4FE8BEF0D848F8`.

All finalMOE/CCRs/gates/ENABLE zero and portsclosed; failedfixtureexit1 retained.
No71repeat orhigherduty. test_drv_range345 validates the two passes and failed
recovery separately. Highest completed3/3cohort remains14FE/340/7.0, not this
new build's7.1. ActualFAA0 installed/stopped. Next assess the retained sustained
outlier and remaining arm budget before proposing another experiment; do not
confuse a10s holdpass with a30s recoverypass or claim hardware is the ceiling.
