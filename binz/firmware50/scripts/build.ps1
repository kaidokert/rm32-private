<#
.SYNOPSIS
    Build firmware50 independently of any ambient cargo configuration.

.DESCRIPTION
    Cargo merges `.cargo/config.toml` from the current directory AND every
    ancestor directory. Because this crate lives under `binz/`, a plain
    `cargo build` inherits binz's `[target.thumbv6m-none-eabi]` settings —
    its linker wrapper and its rustflags — no matter what this crate's own
    config says and no matter which directory the build is launched from.
    That is precisely the coupling this crate is required not to have.

    Environment variables override every config file, so this script pins the
    toolchain explicitly:

      CARGO_TARGET_THUMBV6M_NONE_EABI_LINKER    -> rust-lld from this toolchain
      CARGO_TARGET_THUMBV6M_NONE_EABI_RUSTFLAGS -> just the linker flavor

    `-T link.x` is deliberately absent from the rustflags: build.rs emits it as
    a `cargo:rustc-link-arg`, which travels with the crate. Supplying it in
    both places makes the linker read `memory.x` twice and fail with
    "region 'FLASH' already defined".

.PARAMETER Profile
    Cargo profile. Defaults to `release`.
#>
param(
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

$crate = Split-Path -Parent $PSScriptRoot
Push-Location $crate
try {
    # Resolve rust-lld out of the active toolchain rather than trusting PATH.
    $sysroot = (& rustc --print sysroot).Trim()
    $hostTriple = (& rustc -vV | Select-String '^host: ').Line -replace '^host: ', ''
    $lld = Join-Path $sysroot "lib\rustlib\$hostTriple\bin\rust-lld.exe"
    if (-not (Test-Path $lld)) {
        throw "rust-lld not found at $lld"
    }

    $env:CARGO_TARGET_THUMBV6M_NONE_EABI_LINKER = $lld
    $env:CARGO_TARGET_THUMBV6M_NONE_EABI_RUSTFLAGS = "-C linker-flavor=ld.lld"

    Write-Host "firmware50: building profile '$Profile' for thumbv6m-none-eabi" -ForegroundColor Cyan
    Write-Host "  linker: $lld"

    $profileArgs = if ($Profile -eq "dev") { @() } else { @("--profile", $Profile) }
    & cargo build --target thumbv6m-none-eabi @profileArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $dirName = if ($Profile -eq "dev") { "debug" } else { $Profile }
    $elf = Join-Path $crate "target\thumbv6m-none-eabi\$dirName\shell-pwm"
    if (-not (Test-Path $elf)) { throw "ELF not produced at $elf" }

    Write-Host ""
    Write-Host "=== size ===" -ForegroundColor Cyan
    & arm-none-eabi-size $elf

    Write-Host ""
    Write-Host "=== sections ===" -ForegroundColor Cyan
    & arm-none-eabi-size -A $elf | Select-String -Pattern '\.text|\.rodata|\.data|\.bss|Total'

    Write-Host ""
    Write-Host "=== identity ===" -ForegroundColor Cyan
    $hash = (Get-FileHash -Algorithm SHA256 $elf).Hash
    Write-Host "sha256: $hash"
    Write-Host "path:   $elf"
}
finally {
    Pop-Location
}
