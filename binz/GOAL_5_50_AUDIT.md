# 5–50% goal acceptance audit

Current as of E806 (2026-09-16). This is a gap ledger, not a declaration that
historical results transfer to the final image. Failures remain part of the
curve. Exact hashes and capture names live in the cited reports.

## Duty curve evidence

| duty | strongest retained evidence | status for current goal |
|---:|---|---|
| 5% | E203/E208: two 30 s injected-loss/recovery campaigns near 267.5 eHz | Characterized on historical image; repeat one rung run on final campaign image |
| 10% | Traversed in many live ramps; no dedicated final-image hold/recovery cohort | Missing representative qualification |
| 15% | `LEAN_QUALIFICATION_E762.md`: lean 30 s hold 3/3; independent PSU anchor 230 mA at 11.8 V | Qualified historical representative, useful curve/current anchor |
| 20% | `M0_CLEAN_ENVELOPE_E764_E765.md`: lean 30 s hold 3/3, ~0.92 kHz | Qualified historical rung |
| 25% | `TRACKING_RECOVERY_E775.md`: normal-start recovery 3/3 with ~19.24 s post-restart holds, 1.15–1.18 kHz | Representative recovery qualified on E775; final-image hold still desirable |
| 30% | E772 30 s hold; E791 20 s hold at ~1.17 kHz; retained current-sensitive recovery failure | Characterized, not final representative |
| 35% | E797 smooth-ramp 30 s deadline, ~1.56 kHz, no foldback/tracking/fast-cycle/bus fault | Strong diagnostic exploratory pass, not repeated lean qualification |
| 40% | E797 reached target then2.5A foldback; E803 with4A PSU/3.5A firmware ACKed38, folded38→37, then bus-sag stopped before39 ACK | Power-path boundary retained; not a 40% hold |
| 45% | None | Missing |
| 50% | None | Missing representative hold/recovery qualification |

## Requirement audit

| requirement | evidence | verdict / remaining proof |
|---|---|---|
| Practical BEMF ESC from startup floor through 50% | Reliable low-duty startup/handoff; sustained35%; E803 hard bus-sag stop below40 on4A PSU | Incomplete above35%; powered campaign paused for power-path fold investigation |
| 5% rungs, one retained repeatable run each | Historical5/15/20/25/30 plus current35/40 evidence | Dedicated10,45,50 missing; final curve must normalize build/supply/carrier |
| Keep foldback, bus, nFAULT, tracking and watchdog protections | E803 E799 powered stop: current foldback acted, independent complete-block bus guard won, final safing passed | Present; staged E804 improves foldback magnitude without changing thresholds/stops |
| Qualify10/25/50 with repeated holds and normal-start recovery | E77525% recovery3/3 | 10% and50% missing; 25% final-image hold desirable |
| Lean qualification has no in-ISR diagnostics | E762/E765/E775 reports; staged adaptive E805 omits diagnostic/fast-cycle/event/tail features and passes four-root audit | E805 built but unflashed; runtime lean verification at10/25/50 remains missing |
| Separate low-rate diagnostic timing/CPU/fault margins | Historical timing work; E797 aggregate current/filter output; staged adaptive E806 aggregate CPU image | Image built/audited, but disabled probe-cost and powered35–50 measurements remain missing |
| Attribute firmware/MCU/motor/power limits using stock AM32 same rig | E530 AM32 short responses331/586/741 eHz at10/20/25%; contaminated settings separately reached831 eHz at30% | Hardware exonerated for old342 eHz wall; matched-speed startup and no-load-current table remains missing |
| Well-supported5–50 curve and75% projection | This ledger plus duty reports | Incomplete until45/50, CPU budget, current/power data and matched reference are measured |
| Work only in binz; document portable findings | binz changes and `PORTABLE_WINS.md` | Preserved; do not edit sibling rm32 implementations |

## Immediate staged action

E799 is installed and safely off. With the operator's4A PSU setting, E803
ACKed38%, applied one38->37 current foldback, then stopped on an independent
complete-block bus sag before39% ACK. This satisfies the operator's explicit
pause condition. Do not issue another powered command until that physical
collapse is corroborated/resolved and the operator resumes.

E804 is the unflashed diagnostic successor. It retains the measured warning
magnitude and uses bounded division-free1..5% severity-scaled foldback; E803's
1.853x excursion maps to5%. Thresholds and terminal stops are unchanged. Its
release-s/thin-LTO/codegen1 ELF and four-root M0 audit are frozen under
`captures/reference/duty50_804_adaptive3500/`. When powered work is explicitly
resumed, flash E804 and repeat disabled guard/role preflights before selecting a
bounded causal test. It is not currently permission to rerun40%.

E805 supersedes fixed1% E800 as the separate lean candidate for eventual
10/25/50 qualification. It omits current-maximum, fast-cycle/event and tail
diagnostics and passes the same four-root arithmetic audit. It remains
unflashed until adaptive E804 diagnostic exploration establishes50 safe.

E806 supersedes fixed1% E801 as the separate CPU image. It uses E805's lean
control bodies plus only nested interrupt-union/root counters dumped post-stop;
no event buffer or live UART. Accounting overhead must be measured with
disabled `cpucheck`, so E806 characterizes headroom but cannot qualify E805.
