# E543 — running-counter COM arm candidate (not installed)

Question: does stopping TIM16 on each accepted crossing materially change
commutation timing or the accepted-cycle boundary? E542 identifies a source
difference from AM32, not a demonstrated cause.

Feature bench-com-keep-running omits ONLY initial CR1=0 in
com_timer::Timer::set_and_enable. DIER=0,CNT=0,ARR=timeout,SR=0,NVIC pending
clear,DIER=1,final CR1=1 remain. Init/stop unchanged; initially stopped timers
still start at final write. Not identical to AM32: retains stale-IRQ clearing.
Running counter advances after CNT=0 during subsequent writes; timeout must
not expire before clearing finishes. A real timing change, not automatically
semantics-neutral. No guards, filters, duty or shared-core changes.

Built E537 common features plus bench-com-keep-running, without read-call.
Release-s/thinLTO/codegen1. Frozen captures/reference/comkeep_543/shell-pwm.elf:
65D4783DB480A7542DB55FEB79F650762D906D0D89E89155431DE9EFBB39EC2B.
Emitted math-audit.S/json alongside it. ADC_COMP08001e88..08001e9e writes
DIER/CNT/ARR/SR/NVIC/DIER/CR1, no preceding CR1 clear; literal TIM16base40014400.
Not a measured cycle saving/WCET or no-helper certificate.

Postrun COMARM marker requires fixture --com-keep-running. Two new host tests
refuse missing/unexpected/duplicate/malformed provenance; two comparator-call
regression tests also pass. Not proof of hardware timer behavior.

Actual board remains inline7E55, lastoffE541. No UART/flash/motor E543.
Root ELF now candidate65D4, NOT installed. Qualified baseline remains
captures/reference/compinline_537/shell-pwm.elf.

Before power: own disabled guard/full timing/ADC checks and timer exercise
covering stopped, running, stale UIF/NVIC, stop/rearm cases. Existing comirq
is gated by bench-adc-probes; its32-trial wait does not cover all these cases.
Host tests/build do not satisfy those checks. Use disabled diagnostic build
if needed; no live ISR recorder. Check expiry slack at smallest actual arm,
not only399ticks. Missing timer qualification is explicit, not waived.

If checks pass: one6.9%10s hold with unchanged guards. Failure retires this
intervention; no padding/layout tuning. Pass permits same-point recovery
comparison, not automatic limit expansion. Higher-stage current/timing
evidence remains required; baseline cohort cannot transfer to this hash.

## E544 — disabled qualification and first powered hold pass

Added idle-only comarmcheck: NVIC stays masked, no COM ISR/output authority;
real Timer::set_and_enable exercised16times x4 initialstates x2ARRs(63,399).
States: initialized stopped, running, running with injectedUIF/NVICpending,
stopped after running. Checks register state, no earlyUIF/pending, expiry
within nominal(timeout+1)/2..+20us, arm<=10us, and stopped/nonpending cleanup.
No new live ISR work. Actual128/128PASS,maxarm1us,minexpiry-minus-arm31us.
This is disabled timer behavior, not COM ISR WCET/loaded dispatch latency.
Host check validates exactcases/slack/finaloff; actualcapture+five malformed
variants tested in one passing host test.

New current/root/frozen image DB3B8AAD13F1A715D795299103CD4BD4B6C212B6429C5C88224BBAB23F2F9055
at captures/reference/comkeep_544/shell-pwm.elf; release-s/thinLTO/codegen1,
emittedmathaudit alongside it. E54365D4 superseded, not tested on hardware.
Prior7E55 safed/guardchecked before download, OpenOCD reset (notprobe-rsreset).
Candidate freshguard3/18,fullfive3200preflight CPU2/6/9,ADCroute3checks PASS.
Captures comkeep_544_arm01,guard01,filter01*,adc01.txt retaindisabledchecks.

One planned powered attempt:
```
python scripts/drv_driven_handoff.py --out captures/comkeep_544_start61_hold69_10s_01.txt --ms 10000 --drive-duty 61 --bemf-duty 69 --phase-shift 60 --core-trace 0 --pwm-roles --carrier-hz 20000 --dma-peer --com-keep-running
```
PASS10.000034s,334.776903eHz,20086COM/20086accepted,cycle sigma40.395747us
(includes acceleration),mincycle2895us. Rawpeak307,busmin11020mV,
IRQunion51.295729%,COMPmax40/COM44/commit21/guard20us,stackuntouched3468.
ADC49750delivered versus49751phase records eachvalidate. Finaloffverified,
UARTclosed. Raw SHA256da956eb90d7630b4e9d67fb7e606182222311a68eee67634854465b9078de90d.
No calibratedcurrent/independentqZC claim, no speedlimit/duty change. Onepass
does not prove timing improvement or removalofhigherboundary. Next atmost
samepointrecovery comparison before deciding whether further testiswarranted;
no inherited3/3frombaseline. Currentcandidate remainsinstalled/OFF.

## E545 — recovery passes, boundary still fails; retire variant

Fresh guard3/18PASS, unchanged DB3B candidate. One30s recovery at6.9%
(same E544 command with --ms30000 --dropout --reentry) PASS. Resumed
27.990739s,333.873160eHz,56072COM/accepted,sigma22.184037us,raw344,
bus11032mV,IRQ51.199225%,stack2676,originaldeadline206usspare. Freshseed999
ticks/acquisition6631us. ADC139256delivered/139257phase validated.
Capture comkeep_545_start61_reentry69_30s_01.txt SHA256
271b6c965df61448f2fb7e694206fa7f712f7607f5960b5c4f53a067e79be754.

Then one7.0%10s boundary test, samecommand asE544 except --bemf-duty70,
unchanged350profile/allguards. FAILED CycleTiming12 after2.041397s,
4155COM/4154accepted,raw297,bus11140mV,IRQ51.458750%,ADC10155/phase10156.
Guardstep6 previous2038511/decision2041364 =>2853us below2858us.
Reference preceding5967ticks/refused5715ticks =>2983.5/2857.5us,
two-cyclemean2920.5us,guard-minus-reference -4.5us. Bothcycles/timestamps
remain accepted-event evidence, not independent rotor-speed measurements.
Controlleravg981,lastavg971,interval975,thiszc900,last978,wait244,filter12,
running1/polling0 retained aftersafing. Entire-run acceptedrate339.2766eHz
includes acceleration; do not present it as speed at the fault.
Capture comkeep_545_start61_hold70_10s_01.txt SHA256
d33191347da15f69604e0bbbbe9b5aa09a8e6ec6a91cf5a9c5d017bab628f3c2.

All attempted runs retained, finaloff verified, no retry/threshold change.
Keeping COM running did not remove the short-cycle refusal; retire this
intervention rather than tune its timing. No demonstrated envelope gain.
Restored frozen inline7E55 via download/OpenOCD, fresh guard3/18/finaloffPASS
(comkeep_545_restored_guard01.txt),UARTclosed. Actualboard7E55/OFF; root ELF
still DB3B retired candidate, so do not flash root by assumption. Historical
7E55 3/3qualification remains build-specific. Independent current anchor and
proper distinction between timing-envelope refusal/tracking loss remain open.
