# Build script for both Win64 (x86_64) and Win32 (i686 / WOW64)
$ErrorActionPreference = "Stop"

function Safe-Clear-LockedFile($path) {
    if (Test-Path $path) {
        try {
            Remove-Item $path -Force -ErrorAction Stop
        } catch {
            $backup = "$path." + [guid]::NewGuid().ToString("N") + ".old"
            Move-Item $path $backup -Force -ErrorAction SilentlyContinue
        }
    }
}

Safe-Clear-LockedFile "target\release\keykey_tsf.dll"
Safe-Clear-LockedFile "target\release\keykey_tsf_32.dll"
Safe-Clear-LockedFile "target\i686-pc-windows-msvc\release\keykey_tsf.dll"

Write-Host "=== Building 64-bit (x86_64) KeyKey ===" -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "=== Building 32-bit (i686) KeyKey for Win32 / WOW64 ===" -ForegroundColor Cyan
cargo build --release --target i686-pc-windows-msvc --package keykey-tsf
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Copy-Item target\i686-pc-windows-msvc\release\keykey_tsf.dll target\release\keykey_tsf_32.dll -Force
Write-Host "Successfully built and packaged both 64-bit (keykey_tsf.dll) and 32-bit (keykey_tsf_32.dll)!" -ForegroundColor Green
