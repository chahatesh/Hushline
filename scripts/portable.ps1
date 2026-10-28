# SPDX-License-Identifier: MIT
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $projectRoot 'src-tauri\target\release\hushline.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw 'Run npm run package first.' }
$out = Join-Path $projectRoot 'artifacts'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$stage = Join-Path $out 'hushline-portable'
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'hushline.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $stage
Copy-Item -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_LICENSES.txt') -Destination $stage
Copy-Item -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md') -Destination $stage
Set-Content -LiteralPath (Join-Path $stage 'READ-ME.txt') -Value 'Hush 0.6.0 portable executable. Requires Windows 10/11 x64 and Microsoft Edge WebView2 Runtime. Settings still live in your Windows user app-data directory. API keys use Windows Credential Manager. This is an unsigned pre-release; live acceptance is pending. Read the repository QA report before relying on it.'
Compress-Archive -LiteralPath (Join-Path $stage 'hushline.exe'),(Join-Path $stage 'LICENSE'),(Join-Path $stage 'THIRD_PARTY_LICENSES.txt'),(Join-Path $stage 'THIRD_PARTY_NOTICES.md'),(Join-Path $stage 'READ-ME.txt') -DestinationPath (Join-Path $out 'Hush_0.6.0_x64_portable.zip') -Force
$archive = Join-Path $out 'Hush_0.6.0_x64_portable.zip'
$hashStream = [IO.File]::OpenRead($archive)
$hasher = [Security.Cryptography.SHA256]::Create()
try { $checksum = [BitConverter]::ToString($hasher.ComputeHash($hashStream)).Replace('-', '').ToLowerInvariant() }
finally { $hashStream.Dispose(); $hasher.Dispose() }
[pscustomobject]@{ File = [IO.Path]::GetFileName($archive); SHA256 = $checksum }
