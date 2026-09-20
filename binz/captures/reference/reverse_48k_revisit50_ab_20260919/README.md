# Replacement-motor revisit-at-50 A/B (2026-09-19)

Two protected diagnostic images, **staged only at freeze**. Both use the
reverse 48 kHz cache-owner control path, active nominal-4A signed-current
foldback/hard stop, >5%/three-scan fast bus stop, nFAULT, tracking and
watchdogs. Both include the same eight-scan/accepted-event terminal ring,
`LIVEACK50` timestamp and poststop 50%-epoch revisit counter. No live UART
printing or new ISR division. `release-hybrid`: binz opt-s, dependencies
opt-z, thin LTO, codegen1. Pure revisit admission tests5/5; four ISR-root
soft-arithmetic audits passed for both images.

| image | SHA256 | text/data/BSS | revisit policy |
|---|---|---|---|
| control | `08E0F21A79923EF6E1247028EAB04BABFB601F8E14FBB1619A4F3E4506308656` | 129528/1200/17660 | enabled through 50% |
| off50 | `A07C41EBC8056B65484BCAABBB9224AD7A138767621C917D8BE9CC5EA750772E` | 129688/1200/17660 | enabled below 50%; physical comparator only at 50% |

The exact one-variable control difference is
`bench-running-revisit-off50`: after the real live duty500 transaction has
published, foreground cannot pend a level retry at/above50. Comparator ISR,
real-edge persistence, commutation, current and bus guards are unchanged.
This deliberately does **not** switch revisit off at35: an old-motor off35
attempt lost tracking before reaching the relevant operating point.

Plan: flash/verify explicit G071, run disabled guard/role/duty checks and a
protected10% gate. If the control image reproduces the50% event, run the
same host schedule on off50. Compare last ACK, `LIVEACK50 age_us`,
`REVISIT50 attempts_after_ack/accepts_after_ack`, and time-aligned SAGSCAN/
SAGEVENTS fields—not only pass/fail. A stop before50 or acquisition refusal
is retained and is not a comparison. A clean50 hold in one image is exploratory
until repeated and checked on a lean image.
