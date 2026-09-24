# firmware50 — reproducibility manifest

Everything needed to rebuild this crate's images bit-for-bit, re-run its
gates, and re-derive every number in `LAB_NOTEBOOK.md` from the captures.
Written for campaign 9 step 1; kept current with each flashed image.

## Toolchain, as used for every image in this manifest

| tool | version |
|---|---|
| rustc | 1.96.0 (ac68faa20 2026-05-25) |
| cargo | 1.96.0 (30a34c682 2026-05-25) |
| target | `thumbv6m-none-eabi` |
| probe-rs | 0.28.0 (git `v0.27.0-159-g3c10cd38`) |
| objdump | GNU objdump (Arm GNU Toolchain 14.2.Rel1) 2.43.1.20241119 |
| python | 3.13.1 |
| host target for tests | `x86_64-pc-windows-msvc` |

The crate is its own cargo workspace root (`[workspace]` in `Cargo.toml`), so
the parent tree's **profiles** never apply to it. That is not isolation:
`.cargo/config.toml` is merged from *every* ancestor directory, and
`binz/.cargo/` exists — a scar this bench already has. This crate's own
`.cargo/config.toml` pins the target and the linker wrapper.

## Vendored dependencies — not committed, and how to restore them

`/ref` and `/target` are gitignored (`.gitignore`). `ref/stm32g0xx-hal` is a
pinned clone of the HAL, vendored so the build is offline; `Cargo.toml`
depends on it by path. To restore:

```
cp -r ../ref/stm32g0xx-hal ref/stm32g0xx-hal
cargo vendor ref/vendor        # the PAC and friends
```

`Cargo.lock` **is** committed — force-added, because the parent repo's
`.gitignore` excludes lockfiles and for a binary crate that would have left
the restore recipe unable to pin anything. The step-1 review caught that the
manifest claimed this before it was true.

## Build

```
powershell -NoProfile -File scripts/build.ps1     # production image + size/sha report
cargo build --release                             # all three binaries
cargo build --release --features com-top          # the diagnostic A/B variant
```

Three binaries, from one library:

| binary | what it is | production? |
|---|---|---|
| `shell-pwm` | the production image | yes |
| `edge-capture` | `shell-pwm` + the COMP decision recorder (E121) | no |
| `chain-capture` | `shell-pwm` + the commutation timing chain (E154) | no |
| `sag-capture` | `shell-pwm` + the bus-sag guard's own inputs, two rings (E176) | no |

**A diagnostic image never qualifies another image.** Both recorders are
`const ON: bool` traits (`capture::EdgeLog`, `chain::ChainLog`); production
instantiates `NoLog`/`NoChain` and the recording code folds away, which
`scripts/isr_diff.py` checks per image.

The `com-top` feature moves COMP to NVIC 0x80, below the COM root: a
diagnostic arrangement only (campaign 8 step 6b, refused for production).

## Flash and run

```
probe-rs download --chip STM32G071RBTx --probe 0483:374b:066CFF343433464757233430 <elf>
probe-rs reset    --chip STM32G071RBTx --probe 0483:374b:066CFF343433464757233430
```

Prefer letting the fixture do it, so the capture's recorded hash is
necessarily the image that ran (the reason: a bisect once labelled its
captures with the ELF at the build path rather than the one on the chip):

```
python scripts/bemf_run.py  --elf captures/elf/<image>.elf --flash --command 5 --runs 3 --timeout 140 --label <label>
python scripts/chain_run.py --elf captures/elf/<image>.elf --flash --command J --timeout 150 --label <label>
```

Console is COM41 at 115200. **Nothing is printed while the bridge is driven** —
UART edges couple into the zero-hysteresis comparator — so every report and
dump is written after `safe_off`.

## Gates, and what each one is

| gate | command | what it proves |
|---|---|---|
| four-root arithmetic audit | `python scripts/isr_audit.py --elf <elf> --root <root> --allow-file scripts/audit_allow.json` | no division, no unbounded loop, no panic path reachable from any motor ISR root |
| link-time math audit | runs automatically (`scripts/audit-linker.cmd` as the target linker) | refuses the link if a division helper is reachable from a root |
| longest-path cycles | `python scripts/isr_cycles.py --elf <elf> --root <root> --loop-bound 12 --loop-bound 12 [--fetch-model]` | a **static upper bound**, not a measurement; 0 wait states by default, `--fetch-model` for the G071's 2 |
| changed-root disassembly | `python scripts/isr_diff.py <old.elf> <new.elf>` | the four roots' instructions, addresses normalised, intra-root branch displacements kept |
| structure limits + RAM headroom | `python scripts/structure_report.py` | bin ≤ 1500 lines, ≤ 10 `unsafe`, no `static mut`, no register writes outside `hw/`, no function > 100 lines, **and every built image leaves ≥ 8 KB of the G071's 36 KB for the stack after `.data + .bss`** (E187; the archived images are checked too and named unflashable rather than failing the build) |
| host tests + replay | `cargo test --target x86_64-pc-windows-msvc` | 336 lib + 9 doc; includes the 1536-decision replay of a recorded capture |
| clippy | `cargo clippy --release --bins` and `--features com-top`, plus `--target x86_64-pc-windows-msvc --lib --tests` | zero warnings |
| per-run fixture gates | `scripts/bemf_run.py` / `scripts/cohort.py` | stop reason, hold, forced, the **non-circular** rate identity, refusal witnesses, ladder prerequisites |

Saved gate outputs live in `captures/gates/`, and **are committed** — an entry
that cites an uncommitted artefact cites nothing. Each names the ELFs it
compared.

## The rate identity — which one, and why the other is gone

`BEMFRATE`'s `zc_rate_permille_of_expected` is **circular whenever
`hold_forced = 0`**, which every gated run is (a forced commutation fails its
gates outright): both sides then come from `hold_ms` and `hold_accepted`, and
the value is quantisation of a self-referential ratio — near `floor(S)/S` for
the unrounded mean sector S, within ±1 permille, because each side is
truncated as well. 988 permille at a 80.97 µs sector. It is a **sawtooth**,
not monotone in speed: only the worst case per integer bracket worsens as the
sector shrinks, and a sector that lands on an integer reads 1000. With
`hold_forced != 0` it stops being circular and carries the forced fraction
instead. It is still emitted — the text
format is the fixture's contract — but **nothing gates on it**, and
`report.rs`'s `the_rate_identity_is_only_truncation` pins the arithmetic.

The gate is `cohort.py`'s `rate_vs_coast_permille`: accepted events over the
**matched powered window** (`BEMFTAIL`: raw counts and spans, no rounding,
ending at the last accepted crossing before the stop) against the rotor's own
speed from the **coast that follows**, fitted over full electrical cycles
placed in time from the stop stamp using the firmware-measured
`offset_us + first_us`. Tolerance 1%, unchanged. Its repeatability has been measured **once**, at 25% duty: sd 2.8 permille
over five runs of one image in one session (E155). That is not a property of
the instrument at other rungs — there is **no calibration at 45% or 47.5%**,
which campaign 9 step 4 owes — and the one cross-session observation is a
single pair (≈ 5 permille, E164). Treat both as observations: run A/Bs back to
back, and state the design's minimum detectable difference before reading a
null.

## Images and captures

`captures/elf/` holds every flashed image, keyed by eight hex digits of a hash
of its contents — **and which hash depends on when it was archived**: the
campaign-7-and-earlier names are a SHA-256 prefix, the campaign-9 names
(`7C55B7E0`, `A1906AEC`, `33B695D8`) are a CRC32, because `gates.py` computes
that. Every capture written from E187 on records **both** in its header
(`# elf_crc32`, `# elf_sha256`); older captures carry only the SHA-256, which
is why a reviewer could not tie a dump to the CRC32 the notebook quotes
(E186 §4). Do not assume a name and a header hash are the same function. `captures/` also holds the per-day run captures, `chain/` (timing
chain dumps), `replay/` (decision captures), `asm/` (root disassembly),
`gates/` (saved gate outputs) and `ladder_state.json` (the fixture's per-ELF
rung ladder).

**Images and captures are preserved on disk but not committed** — 114 MB of
ELFs and growing. `scripts/manifest_hashes.py` writes
`captures/MANIFEST-hashes.txt`, the inventory with sizes and SHA-256, which
*is* committed, so any capture or image can be checked for tampering or
identified after the fact.

## Notebook

`LAB_NOTEBOOK.md` is append-only and is the campaign record: before every
build or run, the change and the prediction; after it, the measurements, the
captures and the verdict against that prediction. Independent reviews are
appended verbatim with the responses. Entries are numbered `E<n>`; line
numbers in older entries have drifted, so trust a citation's content over its
line number.
