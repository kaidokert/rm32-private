# Automatic Cortex-M0 arithmetic audit

Every new thumbv6m firmware link now runs scripts/audit-linker.cmd via
.cargo/config.toml. It invokes the selected Rust toolchain's rust-lld, then
arm-none-eabi-objdump -d -S -C on the completed ELF. Python and the GNU Arm
objdump must be on PATH. This launcher is for the current Windows workspace.
Host builds and cargo check do not link M0 firmware and are not audited.
Cargo cache hits retain the existing artifact/report; no new link means no
new scan. To explicitly rescan an existing artifact:

```powershell
python scripts/drv_math_audit.py target/thumbv6m-none-eabi/release/examples/shell-pwm
```

The linker writes <linked-output>.math-audit.S and .math-audit.json alongside
the hashed Cargo output under target/.../examples or deps. JSON includes ELF
SHA256, caller, call-site address, target helper, category and review note.
Cargo may suppress successful linker stderr; inspect the files, not silence.
Audit-tool failure fails the link; findings themselves are advisory pending
review. Nothing auto-approves or deletes a finding.

Categories: integer division/remainder, 64-bit and 128-bit helpers, software
floating point. Calls and direct tail branches are scanned. This is NOT a
complete detector for inline multiword instructions, indirect calls, every
possible compiler helper spelling or timing cost. Full source-interleaved
disassembly is retained for manual review. A helper linked into formatting
code is not automatically a motor-ISR problem; caller context matters.

scripts/math_audit_allowlist.json is initially empty. Reviewed exceptions use
exact caller/target/reason objects; annotated calls remain in the report.
Do not broadly suppress all division or all compiler-builtins. In particular,
an intentional fallback branch may remain in a timing-critical function even
when proven input bounds keep normal execution on the fast branch.

E375 verification: full release/s/thinLTO/codegen1 firmware rebuild succeeds
through the hook. Report SHA matches copied shell-pwm ELF:
148386b19be2abc77e8d33d3c8c015e1d7d0915bb73c4fc50589e47b6b95a8e1.
102 call sites flagged:84division/remainder,18wide64 helpers. No128/float
helper call detected by this matcher; not a claim of no inline wide math.
All313Python tests pass. Rebuilt image NOTFLASHED; actual283E/lastoffE373.
The seed mean's division fallback is visible at0800b28a; inspect its incoming
branch before declaring that optimization removed a normal-path division.
