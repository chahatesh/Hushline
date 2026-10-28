# SPDX-License-Identifier: MIT
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'WindowsSigning.psm1') -Force
Add-Type -AssemblyName System.IO.Compression.FileSystem
$version = Get-HushVersion
$setup = "Hush_${version}_x64-setup.exe"
$portable = "Hush_${version}_x64_portable.zip"
$checksums = @(Get-Content -LiteralPath (Join-Path $Directory 'SHA256SUMS.txt'))
if ($checksums.Count -ne 2) { throw 'Unexpected release checksum manifest.' }
foreach ($name in @($setup, $portable)) {
    $actual = (Get-FileHash -LiteralPath (Join-Path $Directory $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    if (@($checksums | Where-Object { $_ -ceq "$actual  $name" }).Count -ne 1) { throw 'Release checksum mismatch.' }
}
$report = Get-Content -LiteralPath (Join-Path $Directory 'signing-verification.json') -Raw | ConvertFrom-Json
if ($report.version -ne $version -or ($env:GITHUB_SHA -and $report.sourceCommit -ne $env:GITHUB_SHA)) { throw 'Signed release provenance does not match this revision.' }
$signer = Assert-HushTrustedSignature (Join-Path $Directory $setup)
if ($signer -ne $report.thumbprint) { throw 'Installer signer differs from the build report.' }
$temp = Join-Path (Split-Path $PSScriptRoot -Parent) ('artifacts\verify-signed-' + [guid]::NewGuid().ToString('N') + '.exe')
$zip = [IO.Compression.ZipFile]::OpenRead((Join-Path $Directory $portable))
try {
    $allowed = @('hushline.exe','LICENSE','THIRD_PARTY_LICENSES.txt','THIRD_PARTY_NOTICES.md','READ-ME.txt')
    if ($zip.Entries.Count -ne 5 -or @($zip.Entries.FullName | Select-Object -Unique).Count -ne 5 -or @($zip.Entries | Where-Object { $_.FullName -cnotin $allowed }).Count) { throw 'Unexpected portable archive contents.' }
    $entry = $zip.GetEntry('hushline.exe')
    if ($null -eq $entry -or $entry.Length -gt 67108864) { throw 'Invalid portable executable.' }
    [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $temp, $false)
    if ((Assert-HushTrustedSignature $temp) -ne $signer) { throw 'Portable and installer publishers differ.' }
} finally {
    $zip.Dispose()
    if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp }
}
Write-Output 'Distribution checksums and timestamped trusted signatures verified.'
