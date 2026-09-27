The late-arm explanation fits the arithmetic, but identifies the immediate trigger, not its underlying cause.

- At ci=45, level16: wait=(45>>1)−floor(45×16/64)=22−11=11µs; spent=11 exhausts it. Final writes/service remain outside the modeled budget.
- Closed-loop duration was 7.512s; target hold only 11ms. This does not establish sustained 25% operation.
- Sector and phase totals each equal 64,060; adding 33,881 coalesced accepts gives 97,941 exactly. zc_acc and com_count are both two higher; their accounting boundary needs explanation.
- Recheck deltas are 8,647 requests, 150,855 observations, zero retirements. These measure activity, not rescues.
- Tail span agrees: 11,519−126=11,393µs. If 196 timestamps define that span, 195 intervals imply 2,853eHz; using 196 yields 2,867eHz and approximately 997‰ against coast. Endpoint semantics matter.
- Reason15→LateArm and reason28→UnknownGuard mappings, plus the prior 15% result, are asserted rather than independently established here.

Next: inspect existing experiment evidence and the actual arm path; measure event-specific preparation and residual enable latency before selecting an optimization. Do not relax thresholds or repeat/escalate blindly. Zero margin counters provide no distributional evidence.
