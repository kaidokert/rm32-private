The diff resets the watch after `tracking=false`, before `active=true` and IRQ unmasking. This clears prior-run evidence if the foreground lock succeeds; handover separately establishes the clock origin.

Staleness remains strictly `age > limit`: 240 passes; 241 faults with lateness 1. Across wrap, base `u32::MAX−100` plus 241 becomes 140, retaining age 241. Likewise, 481−240=241. `guard_event` tightens the limit before checking the event.

Stop ordering is `det_release → timestamp → take_back → safe_off → comp_mask → carrier restoration`. Within `safe_off`, MOE clears first. Finish performs coast capture, post-run baseline, injection cleanup and preflight reporting before constructing/emitting the report. The snapshot’s printing occurs outside its interrupt mask.

Concrete blockers: None demonstrated by the supplied evidence.

Proof limitations: Reset failure is silently ignored; the unconditional reset claim requires a foreground, nonnested call. The source-order test cannot prove that. The supplied report body does not establish where `guard_record` executes. Its snapshot excludes later `GuardRecord` scalar reads. All 385 host tests passed; audit and instruction comparisons support their stated scopes, not physical timing or hardware behavior. Candidate remains unflashed.
