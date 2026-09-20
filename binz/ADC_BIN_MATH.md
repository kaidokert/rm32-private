# Exact ADC phase bins - E390, 2026-09-14

## E392 installed combined bin-math qualification

Actual9789F836549D2039AAE6F44DC8F2342E2C782D4525EBDB9EB6BF6D4E5AC3AADD.
Includes E391exacttimelineindex and strict BINMATH/--bin-math provenance.
Text122112/data1104/bss29324,release+autoauditPASS,322Python beforehardware.
Priority01threePASSnoUARTrepair,guard01threefaults/18refusals,
preflight01fivePASSCPU2/7/10. No priorities/guard/profilechanges.

binmath_reentry68_30s_01 PASSoriginal30s315.823587eHz53041COM/53040accepted,
IRQ53.246705%,sigma25.968931us,raw278bus11092,COMP59COM63commit37,
recorder35DMA14us versus CC71recorder41DMA18. Stack2696,seed1070/acq6646,
arm49cost12(17spare),deadline209spare,SEEDLAT85usunchanged.
Startupraw521bus11522; initialrelease32us retained. SHA
dbf372fa6d7655240d953094e38c5a942db93abe941af69c497c156e6e08efe7.

Then sameprofile binmath_hold69_01 PASS10s319.911233eHz19195COM/19194accepted,
IRQ53.232947%,sigma41.995847us,raw264bus10901,COMP59COM61commit37,
recorder35DMA14,stack2696. Initialarm79cost12. Startupraw977bus11450,
distinct from running and still below1200rawguard. SHA
ce121a66e2693d4929adf6d44d8818e62ca93984ba6650bd9bf0d5277f56fe53.
Fullfixture/provenance/timing/architecture/chronology/finaloff PASSboth,
portsclosed. E3836.9% refusal remains historical; onechangedbuild pass does
not uniquely prove the earlier cause or repeatability. Next69recovery before
higherduty; no profile increase. Combined changes don't isolate each's cost.

StagedNOTFLASHED9B140D993A452E2E77FA9960F87399FCD2029DD5282FF7FC74B505E242807B91.
ActualCC71 archivedhashverifiedreference/carriermath_cc71,lastoffE389.
No hardware run during this change. Text122000/data1104/bss29324.

Carrier::phase_bin now uses compile-time derived bounded reciprocals:
10k phase*5243>>20,24k phase*805520>>26. The latter reduces phase*32/2666
to phase*16/1333 before deriving reciprocal50345 at2^26. Compiletime error
and overflow assertions prove exact floor for all validphases. Bothruntime
paths use u32 products only. Derivation/proof not runtime work.

Existinghost test strengthened to compare exactold expression for every6399+1
and2665+1 counter value, preserves invalidphase/refusal/balancedbins tests,
and pins memo counterexample1199.212RustPASS, release-s/thinLTO/codegen1 plus
automaticmathauditPASS. The count oftests is unchanged; coverage strengthened.
ADCconsume retainscoherence checks, originalcount/index/flags and phasebound;
explicitperiodmatch now also refuses unsupportedperiod instead of assuming24k.

First candidateA904 emitted unnecessary enumdispatch. Replaced from_ticks
roundtrip with directperiodmatch. Currentcandidate emits multiply/shift instead
of24kdivision; detailedfinalreview/strictcapturemarker still needed beforeflash.
ArchivedCC71consume had one actualuidiv for24k;10k was already strength-reduced,
so don't claim twohelpercalls removed per scan. No measuredCPUimprovement yet.
Operator scheduling concern arrivedduringaudit; next priority is
SCHEDULING_OUTLIER_AUDIT.md. Keep this unflashedcandidate distinct fromCC71.
