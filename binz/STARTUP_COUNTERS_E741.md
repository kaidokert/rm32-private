# E741 — retained polling state, no new powered test

Full goal read. E740 progressed with a reference polling trial. Actual board
remains startpoll_74063b1cf9fcd2be83529e1f09a116a9fde3166fb74c53134b55eaf7652fa98f4bd.
No UART session while SWD connected. Offline gdb ptype and addresses from
that exact ELF identified DriveStore at20000a70, size36, field offsets:
step16,bemf18,min_up20,min_down22,filter24,bad26,rising30,oldroutine31.

OpenOCD init/halt/mdw/resume/shutdown (no reset/write) read nine words:
00000007 00000000 00000000 00000000 00010005 00030003 000b000c
01010000 00000001. Thus zero_crosses7 (six seed + bootstrapCOM), step5,
bemf_count1, min_up/down3, filter12, bad_count11, rising1, oldroutine1,
running1, zcfound0. Last run had19polls and no accepted event. The reference
bad counter retains cumulative mismatches, so 11bad out of19 samples implies
eight matching samples overall, not an entirely absent post-crossing level.
Any mismatch after bad_count>3 resets the BEMF counter; finalcount1 never
met strict >3 threshold. This is retained state, not an ordered level trace
or proof of the physical ZC time, PWM phase, or actual rotor lock.

Fresh off SWD readback: PD1ODR50000c14=0, TIM1BDTR40012c44=00000c1a
(MOE0). Core resumed; no motor commands or reflash.

Found a correctness risk in the experimental E740 watchdog publication:
post-call zero_crosses delta may include a preempting COM increment rather
than a polling qualification. No evidence this race occurred in E740's run
(no published acceptance). Candidate replaces that inference with the exact
reference polling gate/read/count/threshold/latch/zcfoundroutine sequence,
inserting watchdog publication only at its actual qualification decision.
No masking of the reference spin wait, fabricated accepted event, threshold
change, or extra read. Defaults retain shared polling_bemf_check. This local
seam must eventually become a tested shared callback for maintainable parity.
Candidate waits until the sole bootstrap COM has completed before executing
the polling acceptance band. No replacement event is manufactured by that
wait. Release-s/thinLTO build exit0/TIM16 helper audit PASS, root SHA256
fa516d9d355e64bdf51a9733405ef1015e867cb7282d0a45029beb804a5c6a65.
Candidate is NOT installed; adapter equivalence/powered verification remain.

Next evaluate phase-sensitive qualification/startup alignment from these
eight matches/eleven mismatches, not a claim that comparator level is absent.
