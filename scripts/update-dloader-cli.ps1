# Rebuilds the 32-bit r26-cli and copies it into FactoryTool as the Tauri
# sidecar. The sidecar MUST be named with the HOST triple (x86_64) even though
# the binary itself is 32-bit — Tauri selects the sidecar by host triple, and a
# 32-bit child runs fine under a 64-bit parent on Windows (WOW64).
$ErrorActionPreference = "Stop"
$dloader = "D:\code\r26-dloader"
$dest = Join-Path $PSScriptRoot "..\src-tauri\binaries\r26-cli-x86_64-pc-windows-msvc.exe"

Push-Location $dloader
cargo build --release -p r26-cli
Pop-Location

$src = Join-Path $dloader "target\i686-pc-windows-msvc\release\r26-cli.exe"
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
Copy-Item $src $dest -Force
Write-Host "Copied $src -> $dest"
