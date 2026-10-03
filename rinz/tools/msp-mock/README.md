# MSP mock TCP listener

This is a host-side MSPv1 TCP listener for testing Betaflight Configurator's
**Manual Selection** path. It logs requests and replies to the initial identity
queries with a mock `BTFL` controller identity.

It is intentionally not a full Betaflight emulator. The configurator can
establish an MSP connection, but configuration pages will request many commands
that this small mock does not implement.

Run it from this directory:

```powershell
cargo run --target x86_64-pc-windows-msvc
```

Then enter this in the desktop configurator's Manual Selection field:

```text
tcp://127.0.0.1:5761
```

Pass another bind address as the first argument if required. For example,
`cargo run --target x86_64-pc-windows-msvc -- 0.0.0.0:5761` exposes it to the
local network.
