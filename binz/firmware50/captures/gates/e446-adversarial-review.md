Raw establishes a Tracking/Stale stop after 10.170 seconds closed loop and 2.669 seconds at target: watch-feed age 252 µs exceeded the latched 240 µs limit. No forced commutations or late arms were reported; poststop outputs read safe.

The reconstruction gives **14884600 µs** for the final published COMP entry, assuming the clock mapping is valid within one wrap. Attribution to the faulting acceptance is supported by the supplied ordering: publication precedes `guard_event`; `from_event=1` identifies that path; after its trip, the remaining handler code does not publish another acceptance. Merely resuming after shutdown does not invalidate this attribution. The ELF hash alone does not establish source correspondence.

However, **243 µs mixes a current entry timestamp with a previous watch-feed timestamp**. It is neither an edge interval nor evidence of physical slip. The 9 µs is entry-to-watch-decision time, not shutdown latency.

Missing/rejected crossings, revisit-generated acceptance, and software timing remain alternatives. Coast-rate agreement supports rotation, not continuous rotor lock. Zero witness values are unmeasured inputs.

Next, replay the terminal ordering against the supplied code: acceptance publication and timer arm occur **before** stale checking. Verify cancellation and bridge-disable behavior, including pending COM service. Keep thresholds unchanged; this failed run establishes neither shutdown latency nor loaded safety.
