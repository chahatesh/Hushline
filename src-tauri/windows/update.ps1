# SPDX-License-Identifier: MIT
# Embedded in the app. Executed by Windows PowerShell without a profile or a
# change to the machine's execution policy. Paths and version are passed as data.
$ErrorActionPreference = 'Stop'
$stage = $env:HUSH_UPDATE_STAGE
$target = $env:HUSH_UPDATE_EXE
$version = $env:HUSH_UPDATE_VERSION
$setup = Join-Path $stage 'setup.exe'
$directory = Split-Path -LiteralPath $target
$uninstaller = Join-Path $directory 'uninstall.exe'
$oldApp = Join-Path $stage 'old-app.exe'
$oldUninstaller = Join-Path $stage 'old-uninstaller.exe'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\io.chahatesh.hushline'

function Assert-HushSignature([string]$Path) {
    $item = Get-Item -LiteralPath $Path
    if ($item.VersionInfo.ProductName -cne 'Hush' -or
        $item.VersionInfo.ProductVersion -cne $version -or
        $item.VersionInfo.FileVersion -cne $version) { throw 'Unexpected Hush version.' }
    $signature = Microsoft.PowerShell.Security\Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or !$signature.SignerCertificate -or !$signature.TimeStamperCertificate -or
        $signature.SignerCertificate.GetNameInfo([Security.Cryptography.X509Certificates.X509NameType]::SimpleName, $false) -cne 'SignPath Foundation') {
        throw 'Hush requires a trusted, timestamped SignPath Foundation signature.'
    }
    $usages = @($signature.SignerCertificate.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.37' } |
        ForEach-Object { $_.EnhancedKeyUsages } | ForEach-Object { $_.Value })
    if ('1.3.6.1.5.5.7.3.3' -notin $usages) { throw 'Certificate is missing code-signing usage.' }
    $signature.SignerCertificate.Thumbprint
}

function Assert-Installer([IO.Stream]$Stream) {
    if ((Get-FileHash -InputStream $Stream -Algorithm SHA256).Hash -ne $env:HUSH_UPDATE_SHA256) {
        throw 'Installer checksum changed.'
    }
    Assert-HushSignature $setup
}

if ($env:HUSH_UPDATE_MODE -eq 'verify') {
    try {
        $stream = [IO.File]::OpenRead($setup)
        try { $null = Assert-Installer $stream } finally { $stream.Dispose() }
        exit 0
    } catch { exit 4 }
}
if ($env:HUSH_UPDATE_MODE -ne 'install') { exit 1 }

$restore = $false
$keepBackup = $false
try {
    # Wait for dictation and SQLite to shut down before replacing any files.
    $parent = Get-Process -Id ([int]$env:HUSH_UPDATE_PARENT) -ErrorAction SilentlyContinue
    if ($parent -and !$parent.WaitForExit(25000)) { exit 2 }
    $previousRegistration = Get-ItemProperty -LiteralPath $registry -ErrorAction SilentlyContinue
    $previousVersion = $previousRegistration.DisplayVersion
    Copy-Item -LiteralPath $target -Destination $oldApp
    Copy-Item -LiteralPath $uninstaller -Destination $oldUninstaller
    $restore = $true
    # Keep the verified installer open read-only until Windows finishes running
    # it, so another process cannot replace the staged path in between.
    $stream = [IO.File]::OpenRead($setup)
    try {
        $publisher = Assert-Installer $stream
        $arguments = '/S /UPDATE /D=' + $directory
        $installer = Start-Process -FilePath $setup -ArgumentList $arguments -WindowStyle Hidden -PassThru
        if (!$installer.WaitForExit(120000)) {
            Stop-Process -Id $installer.Id -Force -ErrorAction SilentlyContinue
            if (!$installer.WaitForExit(10000)) {
                $keepBackup = $true
                exit 3
            }
            throw 'Installation timed out.'
        }
        if ($installer.ExitCode -ne 0) { throw 'Installation failed.' }
        if ((Assert-HushSignature $target) -ne $publisher -or
            (Assert-HushSignature $uninstaller) -ne $publisher) { throw 'Installed Hush signature does not match.' }
    } finally { $stream.Dispose() }
    $app = Start-Process -FilePath $target -WindowStyle Normal -PassThru
    if ($app.WaitForExit(1500)) { throw 'Updated Hush failed to stay running.' }
    $restore = $false
} catch {
    try {
        if ($restore) {
            Copy-Item -LiteralPath $oldApp -Destination $target -Force
            Copy-Item -LiteralPath $oldUninstaller -Destination $uninstaller -Force
            if ($previousVersion -and (Test-Path -LiteralPath $registry)) {
                Set-ItemProperty -LiteralPath $registry -Name DisplayVersion -Value $previousVersion
            }
        }
        Start-Process -FilePath $target -ArgumentList '--update-failed' -WindowStyle Normal
    } catch { $keepBackup = $restore }
    exit 1
} finally {
    if (!$keepBackup) {
        foreach ($file in @($setup, $oldApp, $oldUninstaller)) {
            if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue }
        }
        try { [IO.Directory]::Delete($stage) } catch { }
    }
}
