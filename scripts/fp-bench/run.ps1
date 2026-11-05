# Downloads the plugins in plugins.txt from Modrinth into cache/ and prints a summary of
# findings per jar, to eyeball false positives. Not part of CI.
# Usage: ./run.ps1 [-List]   (-List only prints what would be downloaded)
param([switch]$List)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$cache = Join-Path $PSScriptRoot 'cache'
$headers = @{ 'User-Agent' = 'jarsnitch-fp-bench' }
# Prefer a Bukkit-family build; fall back to a proxy build.
$loaderSets = @('["paper","spigot","bukkit","folia","purpur"]', '["velocity","bungeecord","waterfall"]')
New-Item -ItemType Directory -Force $cache | Out-Null

$slugs = Get-Content plugins.txt | ForEach-Object { ($_ -split '#')[0].Trim() } | Where-Object { $_ }
foreach ($slug in $slugs) {
    try {
        $version = $null
        foreach ($set in $loaderSets) {
            # Assign before wrapping: Windows PowerShell returns the JSON array as one object.
            $response = Invoke-RestMethod "https://api.modrinth.com/v2/project/$slug/version?loaders=$([uri]::EscapeDataString($set))" -Headers $headers
            $found = @($response)
            if ($found.Count -gt 0) { $version = $found[0]; break }
        }
        if (-not $version) { Write-Warning "${slug}: no server-plugin build on Modrinth"; continue }
        $file = @($version.files | Where-Object { $_.primary })[0]
        if (-not $file) { $file = $version.files[0] }
        if ($List) { "{0,-24} {1,-28} {2}" -f $slug, $version.version_number, $file.url; continue }
        $dest = Join-Path $cache $file.filename
        if (-not (Test-Path $dest)) { Invoke-WebRequest $file.url -OutFile $dest -Headers $headers }
    } catch {
        Write-Warning "${slug}: $_"
    }
}
if ($List) { return }

$json = Join-Path $cache 'reports.json'
cargo run -q --release --manifest-path ../../Cargo.toml -p jarsnitch -- scan $cache --json | Set-Content $json
(Get-Content $json -Raw | ConvertFrom-Json) | ForEach-Object {
    $sev = @($_.findings | ForEach-Object { $_.severity })
    [pscustomobject]@{
        jar     = $_.file.name
        high    = @($sev | Where-Object { $_ -eq 'high' }).Count
        notable = @($sev | Where-Object { $_ -eq 'notable' }).Count
        info    = @($sev | Where-Object { $_ -eq 'info' }).Count
        obf     = $_.obfuscation.score
        rules   = (@($_.findings | Where-Object { $_.severity -ne 'info' } | ForEach-Object { $_.rule }) | Sort-Object -Unique) -join ' '
    }
} | Format-Table -AutoSize -Wrap
