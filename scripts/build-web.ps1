# Builds web/jarsnitch.wasm. Needs the wasm32-unknown-unknown target:
#   rustup target add wasm32-unknown-unknown
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
cargo build -p jarsnitch-wasm --release --target wasm32-unknown-unknown
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Copy-Item target/wasm32-unknown-unknown/release/jarsnitch_wasm.wasm web/jarsnitch.wasm
Write-Host "built web/jarsnitch.wasm ($((Get-Item web/jarsnitch.wasm).Length) bytes)"
# The page's "Try a sample" jars: a made-up plugin from fixtures/ whose code never runs.
New-Item -ItemType Directory -Force web/samples | Out-Null
Copy-Item fixtures/jars/sample-v1.jar web/samples/SampleShop-1.4.0.jar
Copy-Item fixtures/jars/sample-v2.jar web/samples/SampleShop-1.4.1.jar
# The docs pages: docs/guide, docs/*.md and CHANGELOG.md -> web/docs/. Fails on a broken link.
node scripts/build-docs.mjs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
