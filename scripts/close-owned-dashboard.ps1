# SPDX-License-Identifier: MIT
param([Parameter(Mandatory=$true)][int]$ApplicationProcessId, [Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$expected = (Resolve-Path -LiteralPath $Executable).Path
$application = Get-Process -Id $ApplicationProcessId
if ($application.Path -ne $expected -or $application.ProcessName -ne 'hushline') { throw 'Refusing to message an unrelated process.' }
$application.Refresh()
if ($application.MainWindowHandle -eq [IntPtr]::Zero) { throw 'The owned dashboard has no main window.' }
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class HushlineSmokeWindow {
  [DllImport("user32.dll", SetLastError=true)]
  public static extern bool PostMessage(IntPtr window, uint message, UIntPtr wParam, IntPtr lParam);
}
'@
if (-not [HushlineSmokeWindow]::PostMessage($application.MainWindowHandle, 0x0010, [UIntPtr]::Zero, [IntPtr]::Zero)) { throw 'Windows refused the owned dashboard close message.' }
