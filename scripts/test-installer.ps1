# SPDX-License-Identifier: MIT
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$workspaceRoot = [IO.Path]::GetFullPath($projectRoot)
$target = [IO.Path]::GetFullPath((Join-Path $projectRoot 'artifacts\installer-smoke-0.5.2'))
if (-not $target.StartsWith($workspaceRoot + [IO.Path]::DirectorySeparatorChar)) { throw 'Installer target must be inside this workspace.' }
if (Test-Path -LiteralPath $target) { throw 'Test directory already exists; refusing to replace it.' }
if (Get-Process hushline -ErrorAction SilentlyContinue) { throw 'Quit Hushline before the installer test.' }
foreach ($registryRoot in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall','HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall')) {
  $existing = Get-ChildItem $registryRoot -ErrorAction SilentlyContinue | Get-ItemProperty -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -eq 'io.chahatesh.hushline' -or $_.DisplayName -eq 'Hushline' }
  if ($existing) { throw 'An existing Hushline installation was found; refusing to change it.' }
}
if ((Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name Hushline -ErrorAction SilentlyContinue)) { throw 'An existing Hushline startup entry was found; refusing to change it.' }
$installer = Join-Path $projectRoot 'src-tauri\target\release\bundle\nsis\Hush_0.5.2_x64-setup.exe'
# Preserve any unrelated legacy Hush registration and its shortcuts. Our custom
# template uses the unique bundle identifier and "Hush Dictation" shortcut names.
$template = Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri\windows\installer.nsi') -Raw
if (-not $template.Contains('Uninstall\${BUNDLEID}') -or $template.Contains('${PRODUCTNAME}.lnk')) { throw 'Installer identity is not isolated.' }
$legacyKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Hush'
function Read-LegacyRegistration {
  if (Test-Path $legacyKey) { Get-ItemProperty $legacyKey | Select-Object DisplayName,DisplayVersion,UninstallString,Publisher | ConvertTo-Json -Compress } else { '' }
}
$legacyBefore = Read-LegacyRegistration
$legacyLinks = @(
  (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Hush.lnk'),
  (Join-Path ([Environment]::GetFolderPath('Programs')) 'Hush.lnk'),
  (Join-Path ([Environment]::GetFolderPath('Programs')) 'Hush\Hush.lnk')
)
$linkHashes = @{}
foreach ($link in $legacyLinks) { if (Test-Path -LiteralPath $link) { $linkHashes[$link] = (Get-FileHash -LiteralPath $link -Algorithm SHA256).Hash } }
$install = Start-Process -FilePath $installer -ArgumentList '/S',"/D=$target" -WindowStyle Hidden -Wait -PassThru
if ($install.ExitCode -ne 0) { throw "Installer exited with $($install.ExitCode)." }
$executable = Join-Path $target 'hushline.exe'
if (-not (Test-Path -LiteralPath $executable)) { throw 'Installer did not create the expected executable.' }
if (-not (Test-Path -LiteralPath (Join-Path $target '_up_\THIRD_PARTY_LICENSES.txt'))) { throw 'Dependency license bundle is missing from the installation.' }
$process = Start-Process -FilePath $executable -ArgumentList '--background' -WindowStyle Hidden -PassThru
Start-Sleep -Seconds 3
if ($process.HasExited) { throw "Installed app failed to stay running: $($process.ExitCode)." }
Stop-Process -Id $process.Id
$uninstaller = (Resolve-Path -LiteralPath (Join-Path $target 'uninstall.exe')).Path
if (-not $uninstaller.StartsWith($target + [IO.Path]::DirectorySeparatorChar)) { throw 'Refusing to run an uninstaller outside the owned test installation.' }
$uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -Wait -PassThru
$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ((Test-Path -LiteralPath $executable) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
if (Test-Path -LiteralPath $executable) { throw 'Uninstaller left the executable behind.' }
if (Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\io.chahatesh.hushline') { throw 'Uninstaller left its registry entry behind.' }
if ((Read-LegacyRegistration) -ne $legacyBefore) { throw 'The unrelated legacy Hush registration changed.' }
foreach ($link in $linkHashes.Keys) { if (-not (Test-Path -LiteralPath $link) -or (Get-FileHash -LiteralPath $link -Algorithm SHA256).Hash -ne $linkHashes[$link]) { throw 'An unrelated Hush shortcut changed.' } }
$result = [pscustomobject]@{ installed=$true; launchedBackground=$true; dependencyLicensesPresent=$true; uninstalled=$true; legacyHushRegistrationAndShortcutsPreserved=$true; profileRetained=(Test-Path (Join-Path $env:APPDATA 'io.chahatesh.hushline\hushline.sqlite')); target=$target; interactiveWizard='not tested'; liveDictation='not tested' }
$result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $projectRoot 'artifacts\installer-test.json')
$result
