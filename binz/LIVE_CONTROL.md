# Live BEMF terminal control — implementation in progress

Operator clarification: use native MCP or Python, whichever is expedient.
The purpose is flexible exploration, not a transport requirement. Keep the
original finite campaign deadline, electrical/tracking guards, and actual
30% PWM ceiling. No fixed speed objective is imposed; existing speed guards
remain until explicitly requalified.

## Implemented, not yet linked

`examples/support/live_command.rs` is a request-only, allocation-free parser:
`du70` plus CR/LF requests 7%; `?` requests status. The first byte of `off`
or `s`, `!`, ESC, Ctrl-C, malformed input, or a 250ms unfinished command
requests stop immediately. `du0` stops. Values above300 never clamp or wrap.
Blank CR/LF is ignored; successful commands reset parser state. Five host
tests cover every valid duty, malformed/oversized input, stop during each
partial state, original command deadline across timer wrap, and session reset.
This parser grants NO hardware authority. Requests below the eventual writer's
valid minimum must still be refused; parser range is not a motor envelope.

## Required integration

## Hardware functional evidence through E572

Installed029F now supports observed live control: E571 acknowledged70->69 and
ran to deadline; E572 acknowledged70->69->73 then `off` caused HostAbort9
before deadline, finalalloff. Neither run changed guards or firmware.
Use live1 during idle setup, then the established driven campaign; after BEMF
handoff, `?` returns `D=hhhh I=hhhh`, `du73` requests7.3%, `off` stops on its
first byte. Wait for each full reply before the next query/duty command.
Acquisition still aborts on any byte. Duty range4..30% is parser/writer range,
NOT a qualified motor envelope; existing cycle floor still bounds operation.
Recovery campaigns refuse live requests. Pending arm is one-shot and off clears
it. p's idle duty is the startup shell setting, not evidence of last live duty.
E572's23us max covers both COM and writer brackets, not isolated writer WCET.
Stop latency was not measured. No recovery/repeatability or mixed-run sigma
qualification implied. Raw captures live_571_change70_to69 and live_572_up_stop
plus their armACKs retained. Hardware availability supersedes earlier pending
notes below; higher envelope/reliability goal remains open.

## Implementation history

E569 current image029ff46fd77ade89d65b8e46091948fa0a9f711df486d98fb2b1bcd712dd783a
installed (release-s/thinLTO/mathaudit). Explicit off now cancels pending
live_armed. Native MCP live1/live0 acknowledged. Disabled guard3/18, budget5,
filter, atomic256, PWMrole6, CPU2/6/9us, archive and ADCphase checks report
PASS. Transcript captures/live_569_preflight.txt includes final alloff/p/i,
nFAULT1; UART closed. No motor run. This does not cover enabled live writes,
guarded-writer WCET, or UART callback end-to-end during BEMF. Historical staged
8D98/notflashed and41C9installed statements below are superseded by E569.

UART is now staged behind bench-live-control: live1/live0 arms/cancels the next
driven campaign. Each campaign creates a fresh parser/reply buffer. Acquisition
retains any-byte abort; powered+realIRQ admits du<tenths> and ?. RX errors stop,
partial commands timeout, stop bytes are immediate. Reply is16bytes:
`D=012C I=03E8 ` plus CRLF, hexadecimal duty tenths and AM32 half-us interval.
One nonblocking TX attempt per callback, RX first; no flush or live formatter.
Request while previous reply pending stops rather than overwrites. Recovery
campaign updates/status refuse via core readiness. Existing guards/deadlines stay.
Parser5/reply1hosttestsPASS. End-to-end callback and hardware timing UNVERIFIED.

Root8d9855e1300b46293b553e9450ffa19bcaf50a682f652f198ff76b3e67a6863f
release-s/thinLTO build+mathauditPASS, NOTflashed (installed41C9lastoffE568).
Initial flash overflow1504bytes, split disabled livedutycheck from live feature
reduced to224; exclude idle seedcheck harness from livecontrol to fit. Seed
runtime policy/irqbudgetcheck and motor guards unchanged. Diagnostic feature
builds retain their checks. No motor/UART this turn. Need disabled preflights
and actual writer/callback timing before powered use. Older "no UART caller"
notes below describe the preceding staged step, superseded by this paragraph.

Staged `bench-live-control` feature now connects `powered_timer::update_live_duty`
to the PAC transaction and both metadata owners. It checks powered ownership,
ENABLE, reason, real IRQ mode, non-recovery session, old prepared metadata and
fresh guard.poll under one critical section. CCR success publishes PREPARED
and POWER_DUTY before unmasking. Refusals stop; existing fault reason preserved.
The live-prepared latch revokes with bridge cleanup. Matching live commutations
use apply_live_prepared; ordinary/startup paths retain their previous range.
No UART caller yet. Full feature release cargo check passes with pre-existing
warnings; this is NOT linking, emitted-code audit, timing or powered proof.
Next exercise actual guarded writer refusals, metadata/stop races and timing
before connecting opt-in UART. Recovery campaigns currently refuse live updates
instead of silently resuming at their original duty.

Actual writer-body host test added: `python -m unittest scripts.test_live_duty_writer`.
Eleven cases execute the extracted firmware function: success, revoked owner,
latched fault, ENABLE-low, core refusal, stale metadata, absent guard, stale
feedback fault, nFAULT-low, hardware refusal, and over100us duration shutdown.
Mock seams assert mask coverage and exact poll/write/publish order, no metadata
publication on admission refusal, unchanged deadline, preserved first fault,
and wrapping duration accounting. Four Rust tests PASS (writer cohort plus
three actual preload tests). This does NOT exercise real guard policy, core
readiness, IRQ races, PAC hardware, or MCU WCET. No new build/flash/motor.
Shell audit: driven_run's callback at shell-pwm currently aborts on any byte;
the same callback spans acquisition and BEMF. Opt-in parsing must be enabled
only after powered ownership plus real IRQ readiness, never during acquisition.

Next-COM support: `phase_role_sequence::apply_live_prepared` now accepts an
already matching prepared duty40..300, rejects unprepared/stale metadata,
and performs role writes only (no CCR initialization or UG). Not yet called
by the powered owner. Legacy apply/apply_carrier still reject duty>100.
Actual-module host harness `scripts/live_role_tests.rs`:19tests PASS, including
every live duty40..300 in allsix sectors and no-write refusals. This is host
policy evidence only; no new firmware build/flash/motor for this addition.

`live_duty.rs` now supplies a separate prepared-request + preload
transaction. Foreground constructs a private-field request (valid timer,
4..30% duty, exact compare); masked apply has no divide or wait. Mock tests
exercise all valid requests, refusal without mutation, and every combination
of natural wraps across the six update operations, at five duty settings.
All3tests PASS; this is a timer model, not measured PAC/hardware behavior.

Manual evidence: ST RM0444 Rev6 p577, TIM1_CR1 UDIS: disabling update events
retains ARR/PSC/CCRx shadow values. UG can still reinitialize CNT/prescaler,
so the transaction must not issue it. Source:
https://www.st.com/content/ccc/resource/technical/document/reference_manual/group0/2f/21/cb/33/78/80/42/64/DM00371828/files/DM00371828.pdf/jcr%3Acontent/translations/en.DM00371828.pdf
Named local manual was not found; indexed official TIM1 text was retrieved.
Direct whole-PDF fetch stalled and was canceled; not a dependency on downloading
the full manual. PAC adapter is now linked only by the disabled-driver
`bench-live-duty-check` feature; guarded live owner metadata remains pending.

- Opt-in per campaign; preserve legacy any-byte abort when disabled and during
  acquisition. Reset parser/queued requests at stop and before a new campaign.
- Process bounded RX work in foreground. UART errors and partial-command
  timeout stop. Status output must be bounded/nonblocking during drive; the
  post-stop formatter must never be called while the bridge is enabled.
- Compute duty compare in foreground, outside the critical section. Guarded
  writer rechecks active ownership, latched fault, deadlines, feedback age,
  nFAULT, and timer geometry at the actual write. It must not restart the guard,
  reset tracking, extend the session deadline, or increase its speed limit.
- TIM1 CCR1/2/3 preload coherence and phase_role_live PREPARED/core POWER_DUTY
  must change together. Current phase_role_sequence rejects changed duty and
  duty>100. Do NOT just edit POWER_DUTY or reuse commit to create a fake COM.
  Review cached timer manual before choosing update suppression/latching;
  do not use EGR.UG to rephase the carrier or accidentally create a mixed CCR
  update. No source/sink/mux change is required for a duty-only request.
- Extend only this opt-in BEMF path to max300; retain startup limits and
  ordinary fixed-segment behavior. Recovery must not silently restore an old
  pre-exploration duty; either carry the last applied duty with the original
  budget or explicitly stop rather than auto-recover in live mode.
- Test stopped/stale requests, invalid duties, all sectors, timer continuity,
  coherent preloads, stop race, original deadline, and bounded worst-case cost
  while disabled before powered exploration. Audit release-s/LTO emitted code.

E568 installed image is41C9 (full SHA in AGENTS), release-s/thinLTO/audited.
Disabled `livedutycheck`:24/24 cases pass,max2us across six sectors and
4/8/30/7% preloads. UDIS brackets three CCR writes, with no UG/CNT reset;
timer continuity and phase/mux configuration checked. Readback is preload,
NOT active shadow/pulse measurement or powered-handler WCET. First045A test
failed8/24 because its configuration comparison included COMP2.VALUE bit30;
failure transcript retained, corrected comparison masks only that live bit.
Captures: live_duty_568_disabled.txt and live_duty_568b_disabled.txt.
Outputs verified off, UART closed. Live control is not available until integrated
and verified on hardware. Previous quiet run200 failed startup FeedbackStale;
it is not a successful7% hold and not a reason to raise the freshness threshold.
