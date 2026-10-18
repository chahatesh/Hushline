# SPDX-License-Identifier: MIT
# Optional workspace-local toolchain support; standard installations work without this file.
$projectRoot = Split-Path $PSScriptRoot -Parent
$cargoLocal = Join-Path $projectRoot '.tools\cargo'
if (Test-Path $cargoLocal) {
  $env:CARGO_HOME = $cargoLocal
  $env:RUSTUP_HOME = Join-Path $projectRoot '.tools\rustup'
  $env:PATH = "$cargoLocal\bin;$env:PATH"
}
$portableRoot = Join-Path $projectRoot '.tools\msvc'
if (Test-Path $portableRoot) {
  $vcVersion = (Get-ChildItem "$portableRoot\VC\Tools\MSVC" -Directory | Sort-Object Name -Descending | Select-Object -First 1).FullName
  $sdkRoot = Join-Path $portableRoot 'Windows Kits\10'
  $sdkVersion = (Get-ChildItem "$sdkRoot\Include" -Directory | Sort-Object Name -Descending | Select-Object -First 1).Name
  $env:PATH = "$vcVersion\bin\Hostx64\x64;$sdkRoot\bin\$sdkVersion\x64;$sdkRoot\bin\$sdkVersion\x64\ucrt;$env:PATH"
  $env:INCLUDE = "$vcVersion\include;$sdkRoot\Include\$sdkVersion\ucrt;$sdkRoot\Include\$sdkVersion\shared;$sdkRoot\Include\$sdkVersion\um;$sdkRoot\Include\$sdkVersion\winrt"
  $env:LIB = "$vcVersion\lib\x64;$sdkRoot\Lib\$sdkVersion\ucrt\x64;$sdkRoot\Lib\$sdkVersion\um\x64"
  $env:VCToolsInstallDir = "$vcVersion\"
  $env:WindowsSdkDir = "$sdkRoot\"
  $env:WindowsSDKVersion = "$sdkVersion\"
  $env:VSCMD_ARG_TGT_ARCH = 'x64'
  $env:VSCMD_ARG_HOST_ARCH = 'x64'
}
