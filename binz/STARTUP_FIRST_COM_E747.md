# E747: first-COM admission and foreground handoff snapshot

Cause of E746's reason9: powered_timer::commit checked MAX_DUTY_TENTHS100
unless live_matches was already true. The first segment has no live-preload
transaction, so initial200 was rejected and Output::com_step called abort().
This was software admission, not evidence of host transmission or bad wiring.

Timed-startup first-COM ceiling now matches initial-BEMF300; legacy100 remains.
All ownership, sector, current/bus/nFAULT, freshness and tracking checks remain.
Frozen firstcom30_747 SHA256:
8BDF17D119146933405368EE497695F64F7783E9811289E94BF846E9FBE953B6.
Release-s/thinLTO/build0/TIM16audit/download/reset0/guard3/18 PASS.
Capture firstcom30_747_bemf200_ramp300.txt:
measuredseed1600/age250/arm7us; first commit succeeds, tracking stops1005us,
no powered accept. Thus the immediate reason9 is removed; no sustained20%.

Optional bench-handoff-registers diagnostic retains ONE foreground-only
snapshot after the first COM: TIM2 CNT/CR1, COMP CSR, EXTI IMR/RPR/FPR,
NVIC ISER, software masked, polling and step. No ISR hooks, masks or live TX.
Its observer cost is not measured; diagnostic results are not qualification.
First build failed static_mut_refs; fixed via addr_of().read(), no stale flash.
Frozen/installed handoffreg_747b SHA256:
75835561CAF35BCD63AE257FC73355B0BDEE1E132081B0CFDBA7D9F50A64D026.
Release-s/thinLTO/build0/TIM16audit/download/reset0/own guard3/18 PASS.

- handoffreg_747b_quiet_bemf200.txt (phase0, startup62/forced100/BEMF200):
  seed1464/age212/arm7us, 11 commits, latest accepted3500us/sector1;
  reference desync ends observation3558us, average731 vs seed1464.
  Snapshot CNT481, CR1=1, CSR40000271, IMRFFFC0000,
  NVIC00221200, masked0/polling0/step3: TIM2 runs and COMP is unmasked.
  The snapshot clears a permanently masked source as explanation for this run.
- handoffreg_747c_phase60_bemf200.txt: seed1568/age282/arm7us,
  four commits, latest accepted1077us/sector2, tracking stop2106us;
  fresh ADC1834us, average cause0. Snapshot caught the normal masked period
  after an acceptance while waiting for COM, not a stuck mask diagnosis.

All motor tests final-off verified, UART closed. No300 ACK, sustained lock,
qualification transfer, hardware ceiling, or CPU occupancy claim.

After bench tests, source factors the ceiling into const segment_max(timed)
used by first-COM and handoff admission. 33 pure startup tests PASS, including
exhaustive initial40..301 consistency; 21 legacy role tests PASS earlier.
This final helper refactor has NOT been rebuilt/flashed; installed ELF remains
the frozen diagnostic above. Next restore practical low-duty entry followed
by live ramp, comparing the previously successful startup trajectory rather
than mistaking the initial20% jump's transient/desync for a speed envelope.
