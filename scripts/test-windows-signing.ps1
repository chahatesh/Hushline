# SPDX-License-Identifier: MIT
param([string]$BuildDirectory, [switch]$InstallFixture)
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'WindowsSigning.psm1') -Force
$project = Split-Path $PSScriptRoot -Parent
if (-not $BuildDirectory) { $BuildDirectory = Join-Path $project 'src-tauri\target\release' }
$artifacts = Join-Path $project ('artifacts\signing-test-' + [guid]::NewGuid().ToString('N'))
& (Join-Path $PSScriptRoot 'signpath.ps1') -Phase Prepare -BuildDirectory $BuildDirectory -ArtifactDirectory $artifacts
$unsigned = Join-Path $artifacts 'inner-unsigned'
$signed = Join-Path $artifacts 'inner-signed'
New-Item -ItemType Directory -Path $signed | Out-Null
$rsa = [Security.Cryptography.RSA]::Create(2048)
$request = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=Hush signing test only', $rsa, [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pkcs1)
$usages = [Security.Cryptography.OidCollection]::new()
[void]$usages.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'))
$request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($usages, $true))
$certificate = $request.CreateSelfSigned([DateTimeOffset]::UtcNow.AddMinutes(-1), [DateTimeOffset]::UtcNow.AddMinutes(10))
try {
    foreach ($name in @('hushline.exe','uninstall.exe')) {
        $original = Join-Path $unsigned $name
        $candidate = Join-Path $signed $name
        Copy-Item -LiteralPath $original -Destination $candidate
        $null = Set-AuthenticodeSignature -FilePath $candidate -Certificate $certificate -HashAlgorithm SHA256
        Assert-HushPeContent $original $candidate
        $signature = Get-AuthenticodeSignature -LiteralPath $candidate
        if ($null -eq $signature.SignerCertificate -or $signature.SignerCertificate.Subject -ne 'CN=Hush signing test only') { throw 'The temporary fixture was not signed.' }
        $rejected = $false
        try { $null = Assert-HushTrustedSignature $candidate } catch { $rejected = $true }
        if (-not $rejected) { throw 'A test certificate incorrectly passed production verification.' }
    }
    $changed = Join-Path $artifacts 'changed.exe'
    $bytes = [IO.File]::ReadAllBytes((Join-Path $signed 'hushline.exe'))
    $bytes[1000] = $bytes[1000] -bxor 1
    [IO.File]::WriteAllBytes($changed, $bytes)
    $rejected = $false
    try { Assert-HushPeContent (Join-Path $unsigned 'hushline.exe') $changed } catch { $rejected = $true }
    if (-not $rejected) { throw 'Changed executable content incorrectly passed verification.' }
    $rejected = $false
    try { & (Join-Path $PSScriptRoot 'signpath.ps1') -Phase Repack -BuildDirectory $BuildDirectory -ArtifactDirectory $artifacts } catch { $rejected = $true }
    if (-not $rejected) { throw 'Production repacking accepted a test signer.' }
    # Exercise the actual NSIS import path with test-only files. They are kept
    # outside all distribution upload globs and never added to Windows trust.
    $installer = Join-Path $artifacts 'test-only-unsigned-setup.exe'
    Invoke-HushMakensis (Join-Path $BuildDirectory 'nsis\x64\installer.nsi') @("/DHUSH_SIGNED_MAIN=$(Join-Path $signed 'hushline.exe')", "/DHUSH_SIGNED_UNINSTALLER=$(Join-Path $signed 'uninstall.exe')", "/DHUSH_OUTPUT_FILE=$installer")
    Assert-HushMetadata $installer
    if ($InstallFixture) {
        if (Get-Process hushline -ErrorAction SilentlyContinue) { throw 'Refusing installer fixture test while Hush is running.' }
        foreach ($key in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\io.chahatesh.hushline','HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\io.chahatesh.hushline')) {
            if (Test-Path -LiteralPath $key) { throw 'Refusing fixture test over an existing installation.' }
        }
        $target = [IO.Path]::GetFullPath((Join-Path $artifacts 'installed'))
        if (-not $target.StartsWith([IO.Path]::GetFullPath($artifacts) + '\', [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $target)) { throw 'Installer fixture target is not a new owned directory.' }
        $process = Start-Process -FilePath $installer -ArgumentList '/S',"/D=$target" -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw 'Fixture installation failed.' }
        foreach ($name in @('hushline.exe','uninstall.exe')) {
            if ((Get-FileHash -LiteralPath (Join-Path $target $name) -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath (Join-Path $signed $name) -Algorithm SHA256).Hash) { throw 'Installed payload differs from the signed input.' }
        }
        $process = Start-Process -FilePath (Join-Path $target 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -Wait -PassThru
        $deadline = [DateTime]::UtcNow.AddSeconds(15)
        while ((Test-Path -LiteralPath (Join-Path $target 'hushline.exe')) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
        if ((Test-Path -LiteralPath (Join-Path $target 'hushline.exe')) -or (Test-Path -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\io.chahatesh.hushline')) { throw 'Fixture uninstallation left application files or registration.' }
    }
    $report = @{passed=$true;applicationAndUninstallerExport=$true;actualAuthenticodeFixture=$true;unchangedImageBytes=$true;contentTamperingRejected=$true;untrustedSignerRejected=$true;productionRepackRejectedTestSigner=$true;nsisSignedPayloadImportCompiled=$true;windowsTrustModified=$false;privateKeyPersisted=$false;signpathService='not called; enrollment pending';trustedCertificate='not available';installedPayloadHashesAndUninstall=[bool]$InstallFixture;applicationLaunched=$false}
    $report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $artifacts 'test-report.json')
    New-Item -ItemType Directory -Path (Join-Path $project 'artifacts') -Force | Out-Null
    $report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $project 'artifacts\signing-test-report.json')
    Write-Output 'Signing handoff tests passed; untrusted/test signers were correctly refused. Windows trust and the running app were untouched.'
} finally { $certificate.Dispose(); $rsa.Dispose() }
