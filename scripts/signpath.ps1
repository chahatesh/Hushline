# SPDX-License-Identifier: MIT
param(
    [Parameter(Mandatory)][ValidateSet('Prepare','Capture','Repack','Finalize')][string]$Phase,
    [string]$BuildDirectory,
    [string]$ArtifactDirectory,
    [string]$InputFile
)
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'WindowsSigning.psm1') -Force
$project = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
if (-not $BuildDirectory) { $BuildDirectory = Join-Path $project 'src-tauri\target\release' }
if (-not $ArtifactDirectory) { $ArtifactDirectory = Join-Path $project 'artifacts\signpath' }
$build = [IO.Path]::GetFullPath($BuildDirectory)
$artifacts = [IO.Path]::GetFullPath($ArtifactDirectory)
if (-not $build.StartsWith($project + '\', [StringComparison]::OrdinalIgnoreCase) -or -not $artifacts.StartsWith((Join-Path $project 'artifacts') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Signing staging paths must stay inside the workspace and its artifacts directory.' }
$version = Get-HushVersion
$unsigned = Join-Path $artifacts 'inner-unsigned'
$signed = Join-Path $artifacts 'inner-signed'
$setupName = "Hush_${version}_x64-setup.exe"
$script = Join-Path $build 'nsis\x64\installer.nsi'
$manifestPath = Join-Path $artifacts 'unsigned-inputs.json'
function Write-Json($Value, [string]$Path) { [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 8) + "`n", (New-Object Text.UTF8Encoding $false)) }
function Assert-FileSet([string]$Directory, [string[]]$Allowed) {
    if ((Test-Path -LiteralPath $Directory) -and @(Get-ChildItem -LiteralPath $Directory -Force | Where-Object { $_.PSIsContainer -or $_.Name -cnotin $Allowed }).Count) { throw 'Unexpected file in signing staging directory; use a clean owned directory.' }
}
function Assert-Baselines {
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.version -ne $version -or $manifest.installerScript -ne (Get-FileHash -LiteralPath $script -Algorithm SHA256).Hash) { throw 'Installer configuration changed after signing inputs were captured.' }
    foreach ($name in @('hushline.exe','uninstall.exe')) {
        if ($manifest.files.$name -ne (Get-FileHash -LiteralPath (Join-Path $unsigned $name) -Algorithm SHA256).Hash) { throw 'An unsigned signing input changed after capture.' }
    }
}
if ($Phase -eq 'Capture') {
    if (-not $InputFile) { throw 'NSIS did not provide an uninstaller.' }
    # NSIS supplies a temporary .tmp filename, although the content is a PE.
    Copy-Item -LiteralPath $InputFile -Destination (Join-Path $unsigned 'uninstall.exe')
    Assert-HushMetadata (Join-Path $unsigned 'uninstall.exe')
    exit 0
}
if ($Phase -eq 'Prepare') {
    Assert-FileSet $unsigned @('hushline.exe','uninstall.exe')
    New-Item -ItemType Directory -Path $unsigned -Force | Out-Null
    Assert-HushMetadata (Join-Path $build 'hushline.exe')
    if (-not (Select-String -LiteralPath $script -Pattern 'HUSH_EXPORT_UNINSTALLER' -Quiet)) { throw 'Rebuild the installer with the current template before preparing signing inputs.' }
    Invoke-HushMakensis $script @('/DHUSH_EXPORT_UNINSTALLER', "/DHUSH_SIGNING_HELPER=$PSCommandPath", "/DHUSH_SIGNING_DIRECTORY=$artifacts", "/DHUSH_OUTPUT_FILE=$(Join-Path $artifacts 'unsigned-bootstrap-setup.exe')")
    Copy-Item -LiteralPath (Join-Path $build 'hushline.exe') -Destination (Join-Path $unsigned 'hushline.exe')
    $hashes = @{}
    foreach ($name in @('hushline.exe','uninstall.exe')) {
        $path = Join-Path $unsigned $name
        Assert-HushMetadata $path
        if ((Get-AuthenticodeSignature -LiteralPath $path).Status -ne 'NotSigned') { throw 'Expected a fresh unsigned Hush signing input.' }
        $hashes[$name] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    }
    Write-Json @{version=$version;installerScript=(Get-FileHash -LiteralPath $script -Algorithm SHA256).Hash;files=$hashes} $manifestPath
    Write-Output "Prepared Hush $version application and uninstaller for SignPath. No signature has been issued."
    exit 0
}
Assert-Baselines
Assert-FileSet $signed @('hushline.exe','uninstall.exe')
$thumbprints = @()
foreach ($name in @('hushline.exe','uninstall.exe')) { $thumbprints += Assert-HushSignedPair (Join-Path $unsigned $name) (Join-Path $signed $name) }
if (@($thumbprints | Select-Object -Unique).Count -ne 1) { throw 'Hush application and uninstaller must have the same approved signer.' }
$unsignedSetup = Join-Path $artifacts 'installer-unsigned'
if ($Phase -eq 'Repack') {
    Assert-FileSet $unsignedSetup @($setupName)
    New-Item -ItemType Directory -Path $unsignedSetup -Force | Out-Null
    Invoke-HushMakensis $script @("/DHUSH_SIGNED_MAIN=$(Join-Path $signed 'hushline.exe')", "/DHUSH_SIGNED_UNINSTALLER=$(Join-Path $signed 'uninstall.exe')", "/DHUSH_OUTPUT_FILE=$(Join-Path $unsignedSetup $setupName)")
    Assert-HushMetadata (Join-Path $unsignedSetup $setupName)
    Write-Output 'Installer rebuilt with the signed application and signed uninstaller; submit its outer signature next.'
    exit 0
}
$signedSetup = Join-Path (Join-Path $artifacts 'installer-signed') $setupName
$outerThumbprint = Assert-HushSignedPair (Join-Path $unsignedSetup $setupName) $signedSetup
if ($outerThumbprint -ne $thumbprints[0]) { throw 'Installer and payload signing identities differ.' }
$release = Join-Path $artifacts 'release'
Assert-FileSet $release @($setupName, "Hush_${version}_x64_portable.zip", 'SHA256SUMS.txt', 'signing-verification.json')
New-Item -ItemType Directory -Path $release -Force | Out-Null
Copy-Item -LiteralPath $signedSetup -Destination (Join-Path $release $setupName)
& (Join-Path $PSScriptRoot 'portable.ps1') -Executable (Join-Path $signed 'hushline.exe') -OutputDirectory $release -RequireSigned
$files = @($setupName, "Hush_${version}_x64_portable.zip")
$checksums = foreach ($name in $files) { '{0}  {1}' -f (Get-FileHash -LiteralPath (Join-Path $release $name) -Algorithm SHA256).Hash.ToLowerInvariant(), $name }
[IO.File]::WriteAllText((Join-Path $release 'SHA256SUMS.txt'), ($checksums -join "`n") + "`n")
Write-Json @{version=$version;sourceCommit=$env:GITHUB_SHA;publisher='SignPath Foundation';thumbprint=$outerThumbprint;timestampRequired=$true;applicationVerified=$true;uninstallerVerified=$true;installerVerified=$true;unsignedPayloadUnchanged=$true} (Join-Path $release 'signing-verification.json')
Write-Output 'Trusted signatures, timestamps and unchanged payloads verified. Signed distribution files are ready.'
