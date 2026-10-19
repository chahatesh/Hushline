# SPDX-License-Identifier: MIT
param([Parameter(Mandatory=$true)][int]$ApplicationProcessId, [Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$expected = (Resolve-Path -LiteralPath $Executable).Path
$application = Get-Process -Id $ApplicationProcessId
if ($application.Path -ne $expected -or $application.ProcessName -ne 'hushline') { throw 'Refusing to message an unrelated process.' }
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class HushlineSmokeWindow {
  private delegate bool Visitor(IntPtr window, IntPtr param);
  [DllImport("user32.dll")] private static extern bool EnumWindows(Visitor callback, IntPtr param);
  [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr window, StringBuilder text, int maximum);
  [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
  public static IntPtr Dashboard(int process) {
    IntPtr found=IntPtr.Zero;
    EnumWindows((window, param) => {
      uint owner; GetWindowThreadProcessId(window, out owner);
      if(owner != (uint)process || !IsWindowVisible(window)) return true;
      var text=new StringBuilder(512); GetWindowText(window,text,text.Capacity);
      if(text.ToString()=="Hushline") { found=window; return false; }
      return true;
    },IntPtr.Zero);
    return found;
  }
  [DllImport("user32.dll", SetLastError=true)]
  public static extern bool PostMessage(IntPtr window, uint message, UIntPtr wParam, IntPtr lParam);
}
'@
$dashboard = [HushlineSmokeWindow]::Dashboard($ApplicationProcessId)
if ($dashboard -eq [IntPtr]::Zero) { throw 'The owned dashboard has no visible main window.' }
if (-not [HushlineSmokeWindow]::PostMessage($dashboard, 0x0010, [UIntPtr]::Zero, [IntPtr]::Zero)) { throw 'Windows refused the owned dashboard close message.' }
