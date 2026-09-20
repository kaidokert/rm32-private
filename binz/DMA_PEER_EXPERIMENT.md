# DMA-only priority A/B - E399, 2026-09-14

## E401 6.9% recovery still fails - DMA not the sole cause

SameA2DE, dmapeer_reentry69_30s_01: recovered13.088722s near320.006827eHz
then CycleTiming12.25131COM/25130accepted,sigma24.427780us,IRQ53.125180%,
raw263bus10937,DMA22queue2,record19,commit21,stack2676. Seed1058/acq6574,
actualarm47.5us,cost12. Guardstep3 13085672->13088683=3011us<3031 by20us.
Previoussame-sector recorder+29us; reference6473/6018 half-us ticks, paired
mean3122.75us. Last intervalpair1233->838 ticks is redistribution evidence,
not physicalrotor proof. SHA
613753798939262c190582e2e1546f000590b90792550dac79825224b3a56a0a.
Fullfixture correctly rejects, exactfault/finaloffverified; no retry/higherduty.

Source recheck ../minz/core/src/am32_isr.rs99..108: live persistence loop is
outside the critical section beginning109 (mask/reset/COMarm). Guard0 still
can preempt it. DMA A/B does not convict guard or prove analog noise absent;
it rules out DMA preemption as the sole necessary mechanism for this class.
Next assess bounded exclusion around qualification without lowering guard
priority across repeated IRQs. Preserve live reads/reference sequence, prove
mask duration and allow pending guard service between handlers. Not implemented
or qualified yet; don't mask the entire unbounded campaign or weaken guards.

## E400 installed and preceding-point qualified

ActualA2DE7D983DF4F248BCC5C524B17FC52E24D6A8913CE5E4904E08BDD17B896846,
text123648/data1104/bss29336. Actual ADC_COMP/DMA disabled probe bypasses
normal handlers only while diagnosticMODE active; refuses live producers or
pendingvectors. Tests higher-priority nested123, peers132, bothpending213;
restores priorities/enables/pendingempty. dmapeer_priority01 all3trialsPASS,
noUARTrepair. guard01 threefaults/18refusals; preflight01fivePASSCPU2/7/10.
334PythonPASS. Probe MODE checks remain on vectors: not zero observer cost.

dmapeer_hold68_01 PASS10s315.365eHz18922COM/accepted,IRQ53.498%,raw291bus10937,
COMP55COM45commit21,DMA22queue2,stack3412,initialarm78cost12. Startupraw1188
very near1200guard and separate from runningcurrent. SHA
d24c267acbe6e07cf0437f0c68200d77930cf609f7c89d7eee093178f6d75ba8.

dmapeer_reentry68_30s_01 PASSoriginal30s315.567716eHz52999COM/52998accepted,
IRQ53.134900%,cycle sigma24.416767us,raw285bus10972. COMP41COM48commit21,
recorder19,DMA22queue2,stack2676. Seed1071/acq6673,actualarm49cost13,
17usspare,SEEDLAT85us,deadline188spare. Startupraw683. SHA
90c44a77ae717a103463470af182d62c5530d570765287e574618a88192ed5e6.
Fullfixtures/provenance/timing/architecture/finaloffPASSboth, portsclosed.
Same330profile3031/252/allguards. Favorable walltiming vs9789COMP59/COM63/
commit37/recorder35; DMA14->22trade retained. N1, not WCET or causeproof.
Next6.9recovery on samebuild beforehigherduty/profile. Originalstatusbelow
describes E399, not current installed state.

Status: optional bench-dma-peer implemented/built, NOT flashed or qualified.
Installed9789 remains unchanged, lastoff E398 restoration. Both expensive
probes are excluded from the candidate. No motor experiment this entry.

## Why isolate DMA instead of promoting COMP

Existing priorities: COMP64, COM64, guard0, DMA0. Proposed: COMP64, COM64,
guard0, DMA64. No reference/guard algorithm or interrupt workload changes.
The independent100us guard retains priority0 and can preempt all three.
Promoting COMP or lowering the guard could instead starve that safety service;
the count-based64/ms IRQ-rate guard is NOT an elapsed-latency guarantee. It
can tolerate a chain of long handlers across buckets. Do not use it as proof
that a lower-priority guard receives service within200us.

Actual dependency Cargo.lock stm32g0-staging0.17.0, stm32g071/mod.rs enumerates
DMA1_CHANNEL1=9, ADC_COMP=12, TIM6=17, TIM16=21. Under equal priority a pending
DMA has earlier exception-number ordering than COMP/COM; it cannot preempt
their active handler. This ordering must be tested with actual vectors while
outputs are disabled, not inferred from the older TIM2/TIM16 priority probe.

## Risks and safeguards audited

- DMA current check can now wait until an active COMP/COM handler returns.
  Baseline measured maxima59/63us are evidence, not WCET. No guaranteed bound
  is inferred by adding these wall maxima.
- Guard remains priority0; poll checks feedback age1000us and tick gap200us,
  plus nFAULT/tracking/deadline. A stalled lower-priority path does not disable
  this service. Existing global masks remain an unchanged limitation.
- Guard can now interrupt a DMA copy. It does not access ADC STATE/QUEUE.
  Raw copied words stay untrusted until Lease::finish validates flags, NDTR,
  epoch and100us copy bracket. Only then stream_current/FIFO publication run.
- Two pending DMA half-boundaries are ambiguous latches, not two recoverable
  scans. Lease::begin refuses HT|TC; finish refuses a newly pending boundary
  or overwritten half. Do not drain ambiguous data or refresh timestamps.
- Original acquisition timestamps, FIFO overflow refusal,1ms feedback age,
  current/bus limits and100us commit bracket are unchanged. A DMA fault is a
  failed trial, not evidence to weaken these constraints.

## Next hardware contract

1. Add disabled actual ADC_COMP/DMA vector nesting/pending-order probe with
   explicit restore of priorities, enables and pending state. Never run the
   normal handlers against fake ADC data. Keep all peripheral producers off.
2. Read back COMP/COM/DMA64 and guard0. Existing --peer-priority continues to
   expect DMA0 unless --dma-peer explicitly selected; unknown/mixed maps fail.
3. CPU/guard/role/ADC disabled checks unchanged. Only then preceding6.8% hold
   and original30s recovery, exact ADC coherence/current/age/queue evidence.
4. If these pass, one6.9% recovery under same330profile. Retain all refusals;
   no duty/profile increase and no claim that one passing A/B proves cause.

332Python testsPASS, release-s/thinLTO/automatic math auditPASS. Build alone
does not prove scheduling safety. No AM32 run or hardware change implied.
