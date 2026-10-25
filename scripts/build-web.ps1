# Builds web/jarsnitch.wasm. Needs the wasm32-unknown-unknown target:
#   rustup target add wasm32-unknown-unknown
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
cargo build -p jarsnitch-wasm --release --target wasm32-unknown-unknown
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Copy-Item target/wasm32-unknown-unknown/release/jarsnitch_wasm.wasm web/jarsnitch.wasm
Write-Host "built web/jarsnitch.wasm ($((Get-Item web/jarsnitch.wasm).Length) bytes)"
