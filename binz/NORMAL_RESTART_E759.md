# E759 — normal-startup restart policy foundation

The goal requires tracking loss to shut down and recover through the normal
startup path; flying reacquisition must not gate the envelope. Existing code
can shut down on tracking loss but only attempts the optional flying path.

`examples/support/normal_restart.rs` adds a pure one-shot admission policy:

- only powered reason 8 (tracking) may request a restart;
- current, bus, driver, deadline, ADC and execution faults remain latched;
- an attempt is consumed even when refused;
- the full autonomous startup duration plus 200 us dispatch reserve must fit;
- returned drive time is the remainder of the original powered deadline,
  never a fresh window; and
- checked arithmetic rejects wrapping sessions.

Three host tests cover the successful arithmetic, duplicate attempt, every
refusal class, minimum remaining segment, invalid window and wrap. This module
is included by shell-pwm but is not yet connected to gate authority. It does
not itself drive, wake, clear a fault, or claim that restart works on hardware.

Integration must reuse the E758 autonomous startup after complete safing and
coast, acquire a new same-wake zero baseline, reinstall average protection,
and pass the returned remaining duration into the powered owner. It must retain
the first tracking stop before the second attempt resets segment statistics.
No flying-seed path is involved.

The previous turn was progress: autonomous startup reached BEMF twice and
the corrected current guard produced retained higher-duty boundaries. E759 is
implementation progress toward the remaining normal-restart requirement, not
qualification or completion.

The complete E758 feature-set release build succeeds with opt-s, thin LTO and
one codegen unit; direct TIM16 arithmetic-helper audit passes. Root ELF SHA256
`da300e37cf81ca57ba694b01b15b588e810a1615260485c27858ac50d9948a6f`.
It is not frozen or flashed because the new policy has no live caller yet;
the board remains on the verified-off E758 image.

## E760–761 live integration and result

`bench-normal-restart` connects the policy to the shell's powered owner. After
a tracking stop it waits through the existing disabled coast interval, admits
one attempt, reacquires a fresh same-wake zero baseline, reinstalls average
current protection, re-arms the normal driven transfer, and executes the full
autonomous startup staircase. It passes the policy's remaining duration to
the powered owner. The first stopped snapshot is retained before the powered
state is reset. No flying acquisition or retained seed is used.

E760's first attempt was stopped before powered handoff by one isolated
10.05 ms current block (6512 > 4329); the tracking injection and restart were
not exercised. The nominal average guard now requires two consecutive
over-limit blocks: 20.1 ms persistence. A good block resets persistence;
ADC rails remain immediate, as do bus, nFAULT, tracking and execution faults.
The physical threshold remains 1 A. This is a temporal definition of average
current, not a raised count chosen to clear E760. The 34 shared policy tests
and 10 live-adapter tests pass, including persistence reset/latch and rails.

Installed E761 image:
`captures/reference/normalrestart_761/shell-pwm.elf`, SHA256
`057764b9a6c85fae63332296bf69225acdbc50af90d16db003faa5f1d2c020ce`.
Release opt-s/thin LTO/codegen1, direct TIM16 helper audit, download, OpenOCD
reset and disabled guard3/18 all pass.

Capture `captures/normalrestart_761_direct200_30s.txt` proves one injected
normal restart. First powered segment stopped for tracking reason8 at
2,000,605 us after4,057 commutations. Firmware coasted with outputs disabled,
then performed a new autonomous startup. It entered BEMF without a flying seed
and ran the remaining22,979,003 us. This accounts for the off/coast/startup
time inside the original30s powered deadline; no fresh30s budget was issued.
Final current residual1575 < allowance4327, bus/current/driver guards did not
stop, last event and feedback remained fresh, and final output-off verification
passed. Lean-core intentionally omits full accepted-event statistics, so this
is an exploratory functional proof, not qZC/sigma qualification.

One post-test source correction resets the one-shot policy state at each new
external `run` command. Root ELF
`2cb707ad807b6912a77b09cce3f434846eaecc35ec91fb3626a05e46c5e6f265`
builds and passes the same direct TIM16 audit, but is not flashed; therefore
the live proof applies to E761 and a single boot campaign. E761 is safely off.
