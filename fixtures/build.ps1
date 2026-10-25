# Builds fixtures/jars/<name>.jar from each fixtures/src/<name>/ against the API stubs.
# Needs JDK 17 or newer. Output is reproducible: no manifest, fixed timestamps.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$date = '2000-01-01T00:00:02Z'
$javacFlags = @('--release', '17', '-nowarn', '-Xlint:none')

function Invoke-Tool([string]$exe, [string[]]$toolArgs) {
    & $exe @toolArgs
    if ($LASTEXITCODE -ne 0) { throw "$exe failed with exit code $LASTEXITCODE" }
}

if (Test-Path build) { Remove-Item -Recurse -Force build }
New-Item -ItemType Directory -Force build/stubs, jars | Out-Null
Get-ChildItem stubs -Recurse -Filter *.java | ForEach-Object { $_.FullName } | Set-Content build/stubs.txt
Invoke-Tool javac ($javacFlags + @('-d', 'build/stubs', '@build/stubs.txt'))

foreach ($dir in Get-ChildItem src -Directory) {
    $name = $dir.Name
    $out = "build/classes/$name"
    New-Item -ItemType Directory -Force $out | Out-Null
    Get-ChildItem $dir.FullName -Recurse -Filter *.java | ForEach-Object { $_.FullName } | Set-Content "build/$name.txt"
    Invoke-Tool javac ($javacFlags + @('-cp', 'build/stubs', '-d', $out, "@build/$name.txt"))
    # Resources (plugin.yml, configs, pom.properties) keep their paths.
    Get-ChildItem $dir.FullName -Recurse -File | Where-Object { $_.Extension -ne '.java' } | ForEach-Object {
        $dest = Join-Path $out $_.FullName.Substring($dir.FullName.Length + 1)
        New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
        Copy-Item $_.FullName $dest
    }
    $jar = "jars/$name.jar"
    if (Test-Path $jar) { Remove-Item $jar }
    Invoke-Tool jar @('--create', '--no-manifest', "--date=$date", '--file', $jar, '-C', $out, '.')
}

Write-Host "built $((Get-ChildItem jars -Filter *.jar).Count) fixture jars"
