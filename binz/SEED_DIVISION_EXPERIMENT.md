# Exact bounded seed mean — E374, staged only

## E378 lean build, 2026-09-14

Installed4D43E26B76B77B3C44C570ECA13866D67682ED6BFBB1DCD4CE07FF7BD65153A5.
Removed only optionalacquiretiming feature; retain seeddiv/guardinstall and
all prior safety features. Archived6B71 at reference/seeddiv_probe_6b71.
Release/automaticmathauditPASS, text121768,data1104,bss29324; emitted bound,
MUL0800b1ca/LSR18b1cc and exceptional fallbackb20e verified beforeflash.
InitialUARTsilentpriority01 retained; exactdisabledregisters beforeclockrepair.
priority02threePASS,guard01threefaults/18refusals,preflight01fivePASS CPU2/7/10.
seedlean_reentry64_30s_01 fullPASS original30s:292.845476eHz49181COM/49180acc,
IRQ55.207670%,sigma24.714818us,raw278bus10937,COMP65COM69commit45,stack2696.
Seed1151/acq7470us,arm57cost12(25spare),deadline179spare. Startupraw530bus11378.
SEEDLAT44/10/8/15/2/8=87us; optionalACQUIRELATabsent, correctlyunknown inreport.
SHA7fb698ddbf00c91264b3f4d4b52e429bc8d99240f2503feec8e2c02c5a7da8b0.
Fullfixture/SEEDMATH/timing/finaloff verified,portsclosed. N1lean qualification,
notWCET or higherdutyproof. Next65/66 checks before coordinatedprofiledecision.

## E377 measured result

Installed6B71. priority01UARTsilent retained; exactRCC08000000/PD1ODR0/
BDTR0c1a/CCRs0 beforeUSARTclock08040000 repair. priority02threePASS,
guard01threefaults/18refusalsPASS; preflight01fivePASS CPU2/7/10.
seeddiv_reentry64_30s_01 fullPASS original30s injectedloss/freshrecovery:
292.546657eHz49131COM/49130accepted,IRQ54.960206%,sigma24.659004us,
raw276bus10853,COMP65COM69commit45,guardmax21,stack2680.
Recoveryseed1161/acq7486us,arm56cost13(24spare),deadline144spare.
Startupraw511bus11545. SHA
c58c1749f6c5153e7c35cabbe468a3fd6428faddfdb4d922c8fbad057920b2c2.
Fullfixture/provenance/strictACQUIRELATtiming/finaloff PASS; portsclosed.

ACQUIRELAT31/6/3/2/5us versus E37336/5/3/2/5; qualification5us lower.
SEEDLAT47/10/7/15/2/8=89us versus94us. N1 matched-setting comparison,
not exclusive division cost/WCET; measuredseed differs1161vs1147, so increased
actualarm56vs49.5us is not solely a code effect. SteadyIRQ~55% unchanged.
Retain bounded exact math; next remove optionalacquisitionprobe and qualify
the lean build before wider profile work. No extra instrumented64cohort needed.

## E376 emitted-code correction and compile-time derivation

Actual283E remainsinstalled,lastoffE373. Latest candidate6B71A8226E9964FF15287B97A3F5B19064CBCB61C23A79DFB66F15F2495464B3
NOTFLASHED. E375148386 was NOT an effective fastpath: disassembly called
__aeabi_uidiv unconditionally at0800b28a BEFORE the range check. Source
conditional and arithmetic tests alone did not establish optimized execution.

Fallback is now cold/noinline with black_box input confined to that branch.
This preserves exact out-of-domain behavior while preventing speculative
division on accepted sums. Final ELF compares at0800b2ac, branches at0800b2ae,
usesMUL0800b2b2/LSR18 at0800b2b4 on accepted sums, and calls fallback only at
0800b2f6 on the exceptional path. No direct division call in normal seed mean.

Following operator const-math instruction, DIVISOR/MAX_SUM/SHIFT/SCALE and
RECIPROCAL are const; reciprocal ceiling division and error/overflow proof
assertions execute in const evaluation. Emitted path contains literal loads,
multiply/shift, not runtime reciprocal calculation or proof checks. Mean itself
is runtime because intervals are measured. Keep this distinction in later audits.

SEEDMATH fixture marker and --seed-div12 strictly identify the feature;
missing/duplicate/malformed provenance rejects.314Python/207Rust testsPASS,
release/s/thinLTO/codegen1 automaticmathaudit PASS. Text122160,data1120,bss29324.
No hardware timing improvement claimed. Next disabledpreflights then one
matched64recovery with --seed-div12 and existingflags plus strictACQUIRELAT
timingreport, compare E37336usqualification/94usarmage. Allguards unchanged.

Actual installed283E5E7B, lastoffE373. Candidate1C974384B2E0C0F856BAFBC3B6D7BF0A6261EC6D0B5C7388AFD3DD69FF7BE6E9 NOTFLASHED.

E373 edge-to-qualified36us includes software work after mandatory20us dwell.
Generated283E edge routine0800b0f8 calls __aeabi_uidiv at0800b286 to divide
the12-interval sum by12. This is an identified software call, NOT proof that
it accounts for the entire16us remainder or a measured exclusive cost.

Optional bench-seed-div12 computes (sum*21846)>>18 for sum<=24000; larger
values retain sum/12. Every accepted interval is<=2000 by poll(tick), and
there are exactly12 additions. No interval/cycle/dwell/fault/seed-age checks
change. 21846=ceil(2^18/12); maximum approximation error in the accepted
domain is24000*(2/3)/2^18 <1/12, so it cannot round into the next quotient.
Product remains below2^32. Exhaustive test checks all24001 domain values,
plus fallback boundaries/u32::MAX.207Rust testsPASS, release/s/thinLTO/codegen1
with --no-default-features PASS. Current candidate includes acquire timing;
text122040 (+24),data1120,bss29324 unchanged versus283E.

Next inspect generated domain branch for MUL/shift (generic division fallback
must remain), add strict fixture provenance, then disabledpreflights and one
matched64recovery with ACQUIRELAT/SEEDLAT. Compare E37336us qualification and
94us totalarmage, preserving all raw failures. No measured speedup yet; no
profile expansion until hardware outcome. No changes to sibling minz core.
