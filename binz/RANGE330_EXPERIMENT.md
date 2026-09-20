# Bounded330 profile — E380, staged NOT FLASHED

## E381 installed, matched point passes and first67hold passes

ActualF7155BFB installed. Preflashoff verified; priority01UARTsilent retained,
RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 beforeUARTclock08040000 repair.
priority02threePASS,guard01threefaults/18refusals,preflight01fivePASS CPU2/7/10.
range330_hold66_01 PASS10s304.082329eHz18245COM/18244acc,IRQ55.057877%,
raw271bus11128,arm76.5cost13,stack3356;startupraw627bus11545.
SHA7fc5e98402ad90c48744c4b5b691534da869fbcd1533c3e7bfdaa01f9095276f.
range330_reentry66_30s_01 PASSoriginal30s304.043397eHz51062COM/51061acc,
IRQ54.991055%,sigma28.653611us,raw272bus10925,arm52cost12(20spare),
deadline194spare,seed1113/acq7053us,SEEDLAT87us;startupraw757bus11522.
SHA61e016f8c5945dbf00fb3148d250714fa827aecec24e6df85389e12cecddfee2.
Then range330_hold67_01 PASS10s309.987370eHz18599COM/acc,IRQ55.545126%,
sigma39.488256us,raw265bus10913,arm74.5cost12;startupraw760bus11438.
SHA8c5d5bb84f9841a52c556470b00913ca9ceec81f9ef7bf3aacdb483cb9b4b245.
AllCOMP65COM69commit45, recovery/67stack2696. Fullfixtures/provenance/
3031/252readback/finaloffPASS,portsclosed. Next67recovery beforehigherduty.
No330eHz claim, repeatabilitycohort or reducedjitter claim; olderrefusals retained.

CandidateF7155BFB3A5E1CBEEDA8D9CADE8A7F91EFC290093970E7717517102DF279124C.
Installed4D43 remains, lastoffE379. Exact reference archive:
captures/reference/seedlean_4d43/shell-pwm.elf, hash verified before rebuild.

Basis: E378-379 lean seed math measured87us edge-to-arm at64/65/66, with
hold/recovery passes at preceding65/66 and20.5us actual arm spare at66.
This is measured latency, not WCET. At prospective1010tick seed interval,
reference wait253ticks and age174ticks leave79ticks=39.5us,7.5us above32floor.
Compared with earlier91us path, this is4us more nominal boundary margin.

bench-range330 explicitly expands speed-related bounds ONLY:

| Quantity | Old320 | New330 |
| --- | ---: | ---: |
| Runtime same-phase cycle minimum us |3125|3031|
| Runtime individual event minimum us |260|252|
| Acquisition full-cycle minimum half-us ticks |6250|6062|
| Acquisition individual minimum half-us ticks |520|504|
| Minimum seed interval half-us ticks |1041|1010|

3031 is ceil(1e6/330), so full-cycle boundary is conservatively rounded.
Independent actualarm32us/armcost16us, twelve intervals/seven cycles,
current/bus/feedbackage/tracking/deadline/storm/commit protections unchanged.
It is not a jitter fix or retroactive pass for previous6.7% failures.
Host recognizes exact coordinated metadata, rejecting mismatches. Synthetic
relabeled capture test is protocol evidence only. Otherprofiles unchanged.

209Rust/316Python testsPASS; seed rejectsage190ticks but accepts189 exactly
at32us remaining. Uniform1010tick sectors fail6062cycle bound;1011 pass.
Runtime3030us fails3031floor; current3249/age1001/tracking1001 still refuse.
Release/s/thinLTO/codegen1 and automatic math audit PASS; text121780,data1104,
bss29324. No hardware activity or330eHz qualification this entry.

Next disabledpriority/guard/fullpreflight on candidate then matched66 hold/
recovery (samepreceding point) with exactRUNLIMIT3031/252 readback required.
Only after those pass, one bounded67hold; retain anyfailure and stop for its
cause. No automatic extra profile increase to turn a refusal into a pass.
PSU800mA, finitecampaign, max30%duty authorization, allfinaloff checks retained.
