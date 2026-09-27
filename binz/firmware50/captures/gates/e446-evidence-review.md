Raw arithmetic checks: 14,884,609−14,884,357=252 µs, exceeding 240 µs by 12. Mapping the poststop clock pair gives acceptance entry 14,884,566+(28,491−28,457)=14,884,600 µs: 243 µs after the previous watch feed, 9 µs before the decision. This mapping assumes the corresponding clock epoch.

Duty 250 means 25.0%; 64 MHz/1333≈48.012 kHz. Hold rate is 48,325/2.669≈18,106 events/s; 55 µs implies 3030 electrical Hz. Sector counts sum to 137,319, exactly 10,440 below 147,759, matching mailbox coalescing. Tail span is 2,669,906−56=2,669,850 µs, exceeding its advertised 2-second window.

All 14 emitted stale fields are present. Supplied source traces both acceptance paths through publication/arming → `guard_event` → strict stale check → `guard_trip`; reporting separates latched decision fields from poststop snapshots. File provenance and ELF-to-source binding remain unverified.

The latched fields establish an event-origin stale decision. Attribution of the poststop acceptance to that triggering invocation remains conditional: resumed handlers can change published state after shutdown.

Next, audit those continuation paths and the existing ELF’s selected instantiations. This failed attempt establishes reported zero-forced operation and poststop safe readback—not completed duration, rotor lock, physical cause, or bridge-disable latency.
