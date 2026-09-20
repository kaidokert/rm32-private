# Recovery duplicate-clear experiment - E385, 2026-09-14

## E386 hardware result - supersedes staged status below

Actual installed A727F8D7. Assembly review: coast_run_inner local stack204
bytes unchanged versusF715. Initial clear call08005972 retained; resume
discriminant branches080059ac/59b0 skip it; prepare_carrier080059c0 retained.
Text121900,data1104,bss29324. No stack improvement claimed.
priority01 UARTsilent retained; exact RCC08000000/PD1ODR0/BDTR0c1a/CCRs0
before USARTclock08040000 repair. priority02 threePASS; guard01 threefaults,
18poststoprefusals; preflight01 fivePASS, CPU2/7/10us. COM8 terminal untouched.

clearonce_reentry68_30s_01 fullPASS original30s trackingloss/fresh recovery:
316.398247eHz,53138COM/53137accepted, IRQ55.208746%, cycle sigma28.268248us,
raw247counts,bus10937mV, COMP65/COM69/commit45us, stack2696. Startup raw433,
bus11187. Seed1060/acquisition6635us; arm47.5us,cost13us,15.5us abovefloor,
originaldeadline215us spare. SEEDLAT44/9/7/15/2/8=85us versus E38287us.
The2us total improvement includes1us in reset-to-feedback, not solely the
removed clear. N1 favorable wall timing, not exclusive cost/WCET or steady
CPU reduction. No higherduty/profile tested. Outputs-off verified,portclosed.
Capture SHA2562e1c9cf0e8dd0a11417aa4f23edb93e1dc10b54bad6cf5c991a339b6b097cff4.
Next maintain this distinction and pursue larger recovery-path delay only
where ownership and fresh-check reasoning support it; don't infer robust
340+ operation from two microseconds or repeat the failed6.9% blindly.

Staged, NOT flashed: A727F8D7274EA33B7A1B54253E8F5D7639A2A467C7E985A82D5D5E6B1B5C7E6B.
Installed F715 remains unchanged, last verified outputs-off E383. Its exact
ELF is now archived/hash-verified at captures/reference/range330_f715/shell-pwm.elf.

Source audit found two successive physical bridge clears after successful
awake recovery acquisition: flying_bench::acquire_inner clears before return,
then core_bench::coast_run_inner clears again. resume_once between them only
checks original SessionBudget and stores elapsed/window/duty metadata. No
intervening output writes. Failed acquisition/budget paths retain full safing.

Optional bench-reentry-clear-once omits only the second clear when private
coast_run_inner receives resume limits. Initial driven handoff and all other
paths retain their existing clears. Successful acquisition still clears MOE,
CCRs, GPIO latches and restores AF/revokes preparation. Coast entry retains
live ready/output checks, carrier preparation, fresh feedback/age, original
budget admission and measured seed/32us arm floor. No early authority token,
no refreshed edge, no moved guard check, no new unsafe code.

This is a smaller candidate than a new final-edge rendezvous. It does NOT
move carrier preparation early or claim to solve the whole recovery budget.
The expected saved work is one redundant clear/AF restore; actual latency
and codegen effects remain unmeasured. Recovery Limits is currently private
call-path provenance, not a reusable hardware-clear capability: any future
caller or intervening output write must revisit this reasoning.

Release opt-s/thinLTO/codegen1 build PASS with same F715 features plus
bench-reentry-clear-once. Automatic and explicit math audit PASS as tools;
102 advisory helper calls remain, not a clean-math or timing certificate.
318 Python tests PASS. New fixture flag --reentry-clear-once requires exact
REENTRYCLEAR marker; duplicates/malformed/missing required markers refuse.

Next before flash: inspect release branch and stack versus archivedF715,
confirm omission is restricted to resume and initial clear remains. Then
disabled priority/guard/preflight checks and one matched existing6.8% original
30s recovery, with full fixture/provenance/SEEDLAT/finaloff verification.
Compare E382 arm-age87us, reset bracket10us, arm46us and stack2696. Do not
increase duty/profile first. Reject or park if latency worsens; retain failures.
