# Current arithmetic sweep and carrier preparation - E388

## E389 measured carrier candidate -2026-09-14

Actual installedCC71BC841C327AF82EA599C8C528692B3748FD1AE40F51AADD792FF0790203E9.
Adds strict CARRIERMATH fixture marker/--carrier-math to E388 code. Text122056,
data1104,bss29324. Finalgeneratedbranch0800c0cc skipshelper0800c0d4 onprepared
segments. Release+autoauditPASS;320Python beforehardware. Priority01threePASS
withoutUARTrepair, guard01threefaults/18refusals,preflight01fivePASSCPU2/7/10.
Disabledrolecheckallsteps8us versus12-13us earlier; not a motor current claim.

carriermath_reentry68_30s_01 PASSoriginal30s at315.805177eHz,53038COM/accepted,
IRQ54.723812%,sigma27.804355us,raw284bus10937,COMP65COM67commit41,stack2696.
Seed1072/acq6728us,arm49cost13(17spare),deadline191spare,SEEDLAT45/8/7/16/2/7
=85us. Startupraw558bus11522. SHA
d419273233f7b86ed7e6542b98334d1a5778fb77208fd1935a4f79d6dc599892.
Fullfixture/newmarker/timing/architecture/finaloffPASS,portclosed.

Against two A727 matches: commit45->41us,COM69->67us,IRQ55.209/55.509->54.724%.
Comparatorvisit rates differ19.1/19.3->19.4k/s; n1 candidate and code layout/
bounded-successor change prevent assigning whole occupancy difference solely
to carrierdivision. Seedage85usunchanged; no recovery-latency improvement.
No speedprofile increase or6.9% retry. Next cadence-attributed ADCphase math
audit/exact-domain proof; preserve raw measurement/freshguard semantics.

Read GRAYBEARD_M0_ARITHMETIC.md completely on2026-09-14. Its systematic
adapter-layer audit is useful; distinguish source candidates from measured
cost and actual feature-selected cadence. No sibling minz edits.

## Verified corrections / scope

- Current ADC scan rate is4975/s, accepted events about1899/s, COMP visits
  about19k/s. Four divisions per100us guard tick is not established for this
  build. Claimed8-12IRQ percentage-point savings is not measured evidence.
- u8 modulo can be inlined: E387 confirmed2MUL/shift/sub rather than a helper.
- Memo's `(phase*41)>>13` is NOT exact division by200 on0..6399:
  phase1199 yields6 instead of5. Do not deploy it. Any replacement needs an
  exhaustive exact-domain test and same invalid-input refusal behavior.
- powered_timer::commit selects phase_role_live::apply under bench-pwm-roles;
  its sixstep_write call is cfg(not bench-pwm-roles). driven_run::command's
  sixstep_write belongs to guarded forced startup, not sustained BEMF.
  Reusing a startup path is not evidence of a per-COM BEMF EGR.UG regression.
- Recomputing/caching VDDA changes measurement semantics unless rawVREF,
  epoch and freshness equivalence are preserved. It is not automatically a
  zero-risk guard optimization. Retain all current/bus checks meanwhile.

## Implemented candidate: compare only at initial equal-CCR load

phase_role_sequence::apply_carrier previously called carrier.compare on every
commutation, even when prepared duty matched and CCRs were not reloaded.
It now validates duty0/>100 with unchanged Sector/Duty/DutyChanged precedence,
blanks, and calls side-effecting noinline prepare_equal_for_carrier ONLY when
prepared==0. That helper computes the exact original compare and performs the
existing equal-CCR/UG write. The CCRs themselves cache the fixed segment value;
no new shared cached state, owner, output authority or timing policy added.

Tests exhaust100duties x2carriers, first exactcompare, six later role changes
without reload, invalid-duty refusal without writes; existing36transition and
reset/reprepare tests retained.212 Rust tests PASS, release-s/thinLTO/codegen1
and automatic audit PASS. Includes staged E387 bounded-successor cleanup.
Candidate05B73D4C4A60A85EAE5510D265BEF36CAF50ED144AC03D57DCD22C2619CF22CF
NOT FLASHED. Text121944,data1104,bss29324. ActualA727 unchanged,lastoff below.

Generated phase_role_live::apply compares PREPARED at0800c0b2 and branches
over helper at0800c0b4 to0800c0c4. Initial helpercall0800c0bc; division only
inside helper08017282, with originalCCRwrites/UG. No speculation onto steady
prepared calls. TIM16 still has anotherdivide at0800231e (/50); do not claim
division-free ISR or attribute it to carriercompare without path inspection.
Next qualify disabled role/guard/CPU checks and matched recovery before claiming
COM/IRQ savings, then audit remaining emitted live-path divisions by cadence.

## In-flight recovery result retained before switching to memo

InstalledA727 clearonce_reentry68_30s_02 PASSoriginal30s and finaloff:
316.477932eHz53151COM/53150accepted, IRQ55.509313%,sigma28.278701us,
raw291bus10901,COMP65COM69commit45,stack2696. Seed1071/acq6635us,
arm49cost12(17spare),deadline224spare,SEEDLAT44/9/7/15/2/8=85us.
Startupraw694bus11545. SHA
0bfb52e9e6d7134917a1c324d6fb7c8bfe3f0d1915a974b6d38fbf7e05df775a.
Fullfixture/provenance/timing/architecture/finaloff verified,portclosed.
Planned3attempt cohort has2completed/2PASS, third NOT attempted after memo
arrived. Not3/3. No freshPSU response received during this turn.
