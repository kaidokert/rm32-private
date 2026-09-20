# Cycle timing versus qualified envelope - E422

Actual137F/offE421 unchanged. No motor, flash or runtime guard change.

The330profile's3031us same-sector minimum is an experimental speed-envelope
boundary, not a measured silicon limit. E421 refused3017us (about331.46eHz
if interpreted as a cycle), with steady mean319.66eHz. Both numbers describe
accepted-event timing, not an independent rotor measurement. A refusal remains
valid under the current contract; calling it a software bug requires evidence.
Conversely, finding no bug does not prove the hardware cannot run faster.

The operator goal permits coordinated profile expansion after preceding-stage
validation. That is distinct from claiming a guard change fixes jitter. Any
future profile must preserve current/bus/age/stop/32usarm guards and qualify
acquisition plus running timing together, not change only3031 to make a pass.

Prospective335eHz boundary, using actual reused reference advance/wait math:
floor(2e6/(6*335))=995ticks, wait249ticks=124.5us. At observed85/87us seedage,
remaining39.5/37.5us leaves7.5/5.5us above32usfloor. At93us age the same
boundary fails (31.5us remaining). These are arithmetic, NOT WCET or permission
to bypass current admission. Added Rust test proves current1010minimum still
rejects the995seed, and995 is not an enabled alternativeprofile.

Next evidence needed before implementation: inspect the current recovery path
for its variable post-edge work and a conservative deadline bound. The observed
85us mean-like value alone cannot establish335qualification. If no adequate
bound is supported, prepare work earlier while retaining final live admission;
do not keep blaming all threshold-selected tails on hardware or solve them by
an unsupported wider guard. No new range feature is introduced here.
