# SPDX-License-Identifier: MIT
param([string]$Executable, [string]$OutputDirectory, [switch]$RequireSigned)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$exe = if ($Executable) { [IO.Path]::GetFullPath($Executable) } else { Join-Path $projectRoot 'src-tauri\target\release\hushline.exe' }
if (-not (Test-Path -LiteralPath $exe)) { throw 'Run npm run package first.' }
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
if ($RequireSigned) {
    Import-Module (Join-Path $PSScriptRoot 'WindowsSigning.psm1') -Force
    $null = Assert-HushTrustedSignature $exe
}
$artifactRoot = Join-Path $projectRoot 'artifacts'
$out = if ($OutputDirectory) { [IO.Path]::GetFullPath($OutputDirectory) } else { $artifactRoot }
if ($out -ne $artifactRoot -and -not $out.StartsWith($artifactRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Portable output must stay inside the workspace artifacts directory.' }
New-Item -ItemType Directory -Force -Path $out | Out-Null
$stage = Join-Path $artifactRoot 'hushline-portable'
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'hushline.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $stage
Copy-Item -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_LICENSES.txt') -Destination $stage
Copy-Item -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md') -Destination $stage
$signingStatus = if ($RequireSigned) { 'The executable has a verified, timestamped SignPath Foundation Authenticode signature. Signing verifies its publisher and integrity; read the QA report for functional acceptance status.' } else { 'This is an unsigned pre-release; live acceptance is pending.' }
Set-Content -LiteralPath (Join-Path $stage 'READ-ME.txt') -Value "Hush $version portable executable. Requires Windows 10/11 x64 and Microsoft Edge WebView2 Runtime. Settings still live in your Windows user app-data directory. API keys use Windows Credential Manager. $signingStatus Read https://github.com/chahatesh/Hushline/blob/main/CODE_SIGNING_POLICY.md for signing status and verification."
$archive = Join-Path $out "Hush_${version}_x64_portable.zip"
Compress-Archive -LiteralPath (Join-Path $stage 'hushline.exe'),(Join-Path $stage 'LICENSE'),(Join-Path $stage 'THIRD_PARTY_LICENSES.txt'),(Join-Path $stage 'THIRD_PARTY_NOTICES.md'),(Join-Path $stage 'READ-ME.txt') -DestinationPath $archive -Force
$hashStream = [IO.File]::OpenRead($archive)
$hasher = [Security.Cryptography.SHA256]::Create()
try { $checksum = [BitConverter]::ToString($hasher.ComputeHash($hashStream)).Replace('-', '').ToLowerInvariant() }
finally { $hashStream.Dispose(); $hasher.Dispose() }
[pscustomobject]@{ File = [IO.Path]::GetFileName($archive); SHA256 = $checksum }
