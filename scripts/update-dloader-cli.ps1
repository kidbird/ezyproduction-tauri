# Rebuilds the 32-bit r26-cli and copies it into FactoryTool as the Tauri
# sidecar. The sidecar MUST be named with the HOST triple (x86_64) even though
# the binary itself is 32-bit — Tauri selects the sidecar by host triple, and a
# 32-bit child runs fine under a 64-bit parent on Windows (WOW64).
#
# Also writes binaries/r26-cli.version.txt (version + source commit) so the
# vendored blob stays auditable; keep SIDECAR_EXPECTED_VERSION in
# src-tauri/src/dloader.rs in sync when the version changes.
param(
    # Path to the r26-dloader checkout; override with -DloaderPath or the
    # R26_DLOADER_PATH environment variable.
    [string]$DloaderPath = $(if ($env:R26_DLOADER_PATH) { $env:R26_DLOADER_PATH } else { "D:\code\r26-dloader" })
)
$ErrorActionPreference = "Stop"

if (-not (Test-Path (Join-Path $DloaderPath "crates\r26-cli"))) {
    Write-Error "r26-dloader checkout not found at '$DloaderPath'. Pass -DloaderPath or set R26_DLOADER_PATH."
}

$destDir = Join-Path $PSScriptRoot "..\src-tauri\binaries"
$dest = Join-Path $destDir "r26-cli-x86_64-pc-windows-msvc.exe"

Push-Location $DloaderPath
cargo build --release -p r26-cli --target i686-pc-windows-msvc
Pop-Location

$src = Join-Path $DloaderPath "target\i686-pc-windows-msvc\release\r26-cli.exe"
New-Item -ItemType Directory -Force $destDir | Out-Null
Copy-Item $src $dest -Force

# Version stamp: which r26-dloader commit produced this blob, and the version
# string dloader.rs verifies at runtime via `r26-cli --version`.
$sha = git -C $DloaderPath rev-parse HEAD
$ver = (& $dest --version | Out-String).Trim()
$stamp = Join-Path $destDir "r26-cli.version.txt"
@(
    "version: $ver"
    "source-commit: $sha"
    "built: $(Get-Date -Format o)"
) | Set-Content $stamp

Write-Host "Copied $src -> $dest"
Write-Host "Stamped $stamp ($ver @ $($sha.Substring(0, 9)))"
