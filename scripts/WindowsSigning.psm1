# SPDX-License-Identifier: MIT
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-HushVersion {
    (Get-Content -LiteralPath (Join-Path (Split-Path $PSScriptRoot -Parent) 'package.json') -Raw | ConvertFrom-Json).version
}
function Assert-HushMetadata([string]$Path, [string]$Version = (Get-HushVersion)) {
    $item = Get-Item -LiteralPath $Path
    if ($item.Length -lt 512 -or $item.Length -gt 67108864 -or $item.Extension -ne '.exe') { throw 'Unexpected signing input size or file type.' }
    if ($item.VersionInfo.ProductName -ne 'Hush' -or $item.VersionInfo.ProductVersion -ne $Version -or $item.VersionInfo.FileVersion -ne $Version) { throw 'Signing input must be a Hush executable with the exact release version.' }
}
function Get-HushPeLayout([byte[]]$Bytes) {
    if ($Bytes.Length -lt 512 -or $Bytes[0] -ne 0x4d -or $Bytes[1] -ne 0x5a) { throw 'Invalid PE image.' }
    $pe = [BitConverter]::ToUInt32($Bytes, 0x3c)
    if ($pe -gt $Bytes.Length - 160 -or [BitConverter]::ToUInt32($Bytes, $pe) -ne 0x4550) { throw 'Invalid PE header.' }
    $optional = $pe + 24
    $magic = [BitConverter]::ToUInt16($Bytes, $optional)
    if ($magic -eq 0x10b) { $directory = $optional + 96 } elseif ($magic -eq 0x20b) { $directory = $optional + 112 } else { throw 'Unsupported PE image.' }
    $security = $directory + 32
    if ($security + 8 -gt $Bytes.Length) { throw 'Invalid certificate directory.' }
    @{ Checksum = $optional + 64; Security = $security; Offset = [BitConverter]::ToUInt32($Bytes, $security); Size = [BitConverter]::ToUInt32($Bytes, $security + 4) }
}
function Assert-HushPeContent([string]$Unsigned, [string]$Signed) {
    # Authenticode changes only CheckSum, the certificate directory, alignment
    # padding and the appended certificate table. Every other byte must match.
    $before = [IO.File]::ReadAllBytes($Unsigned)
    $after = [IO.File]::ReadAllBytes($Signed)
    $old = Get-HushPeLayout $before
    $new = Get-HushPeLayout $after
    if ($old.Offset -ne 0 -or $old.Size -ne 0) { throw 'The signing baseline must be unsigned.' }
    if ($old.Checksum -ne $new.Checksum -or $old.Security -ne $new.Security -or $new.Offset -lt $before.Length -or $new.Offset -gt $before.Length + 7 -or $new.Offset % 8 -ne 0 -or $new.Size -lt 8 -or [long]$new.Offset + $new.Size -ne $after.Length) { throw 'Unexpected signed PE structure.' }
    # Native byte comparisons avoid millions of PowerShell pipeline iterations.
    $segments = @(@(0, $old.Checksum), @(($old.Checksum + 4), ($old.Security - $old.Checksum - 4)), @(($old.Security + 8), ($before.Length - $old.Security - 8)))
    $hash = [Security.Cryptography.SHA256]::Create()
    try {
        foreach ($segment in $segments) {
            $left = $hash.ComputeHash($before, [int]$segment[0], [int]$segment[1])
            $right = $hash.ComputeHash($after, [int]$segment[0], [int]$segment[1])
            if ([Convert]::ToBase64String($left) -cne [Convert]::ToBase64String($right)) { throw 'Signed file content differs from the approved unsigned input.' }
        }
    } finally { $hash.Dispose() }
    for ($index = $before.Length; $index -lt $new.Offset; $index++) { if ($after[$index] -ne 0) { throw 'Unexpected data before the signature.' } }
}
function Assert-HushTrustedSignature([string]$Path, [string]$Publisher = 'SignPath Foundation') {
    Assert-HushMetadata $Path
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or $null -eq $signature.SignerCertificate) { throw 'A valid, publicly trusted Authenticode signature is required. Unsigned and test certificates cannot be released.' }
    if ($signature.SignerCertificate.GetNameInfo([Security.Cryptography.X509Certificates.X509NameType]::SimpleName, $false) -cne $Publisher) { throw 'The Authenticode publisher does not match the approved signing identity.' }
    if ($null -eq $signature.TimeStamperCertificate) { throw 'A trusted signing timestamp is required.' }
    $eku = @($signature.SignerCertificate.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.37' } | ForEach-Object { $_.EnhancedKeyUsages } | ForEach-Object { $_.Value })
    if ('1.3.6.1.5.5.7.3.3' -notin $eku) { throw 'The certificate is not authorized for code signing.' }
    $signature.SignerCertificate.Thumbprint
}
function Assert-HushSignedPair([string]$Unsigned, [string]$Signed) {
    Assert-HushMetadata $Unsigned
    Assert-HushMetadata $Signed
    Assert-HushPeContent $Unsigned $Signed
    Assert-HushTrustedSignature $Signed
}
function Get-HushMakensis {
    $base = Join-Path $env:LOCALAPPDATA 'tauri\NSIS'
    foreach ($candidate in @((Join-Path $base 'Bin\makensis.exe'), (Join-Path $base 'makensis.exe'))) {
        if (Test-Path -LiteralPath $candidate) { return $candidate }
    }
    $command = Get-Command makensis.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    throw 'Build an NSIS installer with Tauri first; makensis was not found.'
}
function Invoke-HushMakensis([string]$Script, [string[]]$Defines) {
    $compiler = Get-HushMakensis
    Push-Location (Split-Path $Script -Parent)
    try {
        & $compiler '/V2' @Defines $Script
        if ($LASTEXITCODE -ne 0) { throw 'NSIS signing handoff compilation failed.' }
    } finally { Pop-Location }
}
Export-ModuleMember -Function Get-HushVersion, Assert-HushMetadata, Assert-HushPeContent, Assert-HushTrustedSignature, Assert-HushSignedPair, Invoke-HushMakensis
