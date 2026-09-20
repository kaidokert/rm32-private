# Recovery acquisition timing probe — E372

## E373 hardware measurement

Installed283E5E7B. Acquisition localframe508bytes unchanged. Pref lashUARToff
verified; afterreset priority01UARTsilent retained. Exact RCC08000000,
PD1ODR0,BDTR0c1a,CCRs0 checked before USARTclock08040000 repair.
priority02threePASS,guard01threefaults/18poststoprefusalsPASS,preflight01
allfivePASS,CPU2/7/10us. No motor authority during these checks.

acquiretiming_reentry64_30s_01 PASS original30s injected-loss recovery:
292.832646eHz49178COM/49178accepted,IRQ54.784513%,sigma24.727751us,
raw263bus10853,COMP65COM69commit45,stack2680,arm49.5cost12,
deadline150usspare. Startupraw868bus11569; seed1147/acq7408us.
RawSHA069bdc27e874ed347f21f456f8abab31015b0c1aa754d344ff3fd9190d96722a.
Fullfixture/provenance/finaloff and stricttimingreport PASS;311PythonPASS
including exact retained-stage regression. Portsclosed.

ACQUIRELAT ticks14744/14816/14826/14832/14836 gives36/5/3/2us for
edge-to-qualified/qualified-to-clear/clear-to-publish/publish-to-return.
Return-to-entry5us; totaledge-to-entry51us. SEEDLAT51/10/8/15/2/8=94us,
versus baseline91us. N1 instrumented comparison, not isolated probe-cost or
WCET proof. The36us includes20us mandatory confirmation and qualification
bookkeeping; do not call remaining16us removable. Clear/report account8us,
so avoiding caller return/report cannot recover the whole prior29us remainder.
Next inspect final-candidate confirmation/Acquisition::edge bookkeeping with
all twelve intervals/seven cycles/dwell checks preserved. Do not move guard
admission or widenprofile based on this diagnostic. Actual283E remains
installed;957F archived reference retains wider qualification.

Staged only, NOT FLASHED. Candidate283E5E7B350D1AFB097507252AD6AEE3B8580997DD03B5B6CD3F333AE9645617.
Installed957F remains qualified, last outputs-off E369; exact archive at
captures/reference/guardinstall_957f/shell-pwm.elf (hash verified before build).

Optional bench-acquire-timing adds recovery-only timestamps in TIM17 origin
half-us units: measured edge, after successful qualification loop, after
physical clearing, after report publication, before wrapper return. They
never replace the seed/feedback timestamps or influence ownership/admission.
No extra reads on the acquisition scanning loop or steady-running ISR path.
Four clock reads occur on the successful awake return path; their latency
and compiler layout effects are NOT yet measured. Default feature-off behavior
retains no new instrumentation. Successful disabled-only diagnostics do not
emit the marker; failed recovery retains existing failure evidence.

ACQUIRELAT is emitted only by capture-enabled recovery_summary after operation,
not during motor work or ordinary terminal output. drv_timing_report validates
optional singleton exact shape, monotonic origin ticks below20ms, unchanged
20us dwell minimum, and return age bounded by existing SEEDLAT entry age.
It reports five brackets including return-to-entry, explicitly instrumented
and non-exclusive; it never labels them removable delay. Missing marker stays
unknown for old captures. Synthetic parser tests do not prove hardware timing.

310Python tests PASS. Release/s/thinLTO/codegen1 build PASS with the E367 feature
set plus bench-acquire-timing and --no-default-features. Initial build command
omitted --no-default-features and correctly failed incompatible atomic backends;
no hardware effect. Corrected build text122016,data1120,bss29324: +388text,
+16data versus installed957F. Baseline acquire_inner reserves508 localstack
bytes plus saved registers, confirming a continuation would retain a material
frame during powered operation. Candidate frame/observer timing remain to check.

Next: inspect candidate code/stack, flash only after recording safe state,
run existing disabled priority/guard/pulse/atomic/roles/CPU/archive preflights,
then one matched6.4% original30s injected recovery with all prior fixture flags.
Require ACQUIRELAT through timing reporter in addition to full fixture/provenance;
unchanged32us arm floor,16us armcost, current/bus/age/deadline/finaloff gates.
Compare total SEEDLAT to91us baseline to expose probe cost; do not subtract
an assumed read cost. Stop if qualification fails, preserve raw result, no
profile increase or further equivalent cohorts for this diagnostic.
