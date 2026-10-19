# SPDX-License-Identifier: MIT
param([string]$Executable = "$PSScriptRoot\..\src-tauri\target\release\hushline.exe", [int]$Seconds = 20)
$ErrorActionPreference = 'Stop'
$binary = (Resolve-Path -LiteralPath $Executable).Path
if (Get-Process hushline -ErrorAction SilentlyContinue) { throw 'Quit Hushline before benchmarking, to avoid measuring an existing instance.' }
$out = Join-Path (Split-Path $PSScriptRoot -Parent) 'artifacts'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$clock = [Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath $binary -ArgumentList '--background' -WindowStyle Hidden -PassThru
try {
  Start-Sleep -Seconds 5
  if ($proc.HasExited) { throw "Hushline exited with code $($proc.ExitCode); startup failed." }
  $ids = [Collections.Generic.HashSet[int]]::new()
  [void]$ids.Add($proc.Id)
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId
  do { $added = $false; foreach ($entry in $all) { if ($ids.Contains([int]$entry.ParentProcessId) -and $ids.Add([int]$entry.ProcessId)) { $added = $true } } } while ($added)
  $startCpu = 0.0
  foreach ($processId in $ids) { $p = Get-Process -Id $processId -ErrorAction SilentlyContinue; if ($p) { $startCpu += $p.CPU } }
  $sampleClock = [Diagnostics.Stopwatch]::StartNew()
  $samples = @()
  for ($i = 0; $i -lt $Seconds; $i++) {
    $working = 0L; $private = 0L
    foreach ($processId in $ids) { $p = Get-Process -Id $processId -ErrorAction SilentlyContinue; if ($p) { $working += $p.WorkingSet64; $private += $p.PrivateMemorySize64 } }
    $samples += [pscustomobject]@{ workingSetMB=[math]::Round($working/1MB,2); privateMB=[math]::Round($private/1MB,2) }
    Start-Sleep -Seconds 1
  }
  $endCpu = 0.0
  foreach ($processId in $ids) { $p=Get-Process -Id $processId -ErrorAction SilentlyContinue; if ($p) { $endCpu += $p.CPU } }
  $hardware = Get-CimInstance Win32_ComputerSystem
  $processor = Get-CimInstance Win32_Processor | Select-Object -First 1
  $report = [pscustomobject]@{
    measuredAt=(Get-Date).ToUniversalTime().ToString('o'); executable=$binary; binarySha256=(Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash; configuration='release, background tray, no webviews created, resting native dock visible; no offline model resident';
    cpu=$processor.Name; logicalProcessors=[Environment]::ProcessorCount; systemMemoryGB=[math]::Round($hardware.TotalPhysicalMemory/1GB,1); os=[Environment]::OSVersion.VersionString;
    warmupSeconds=5; sampleSeconds=$sampleClock.Elapsed.TotalSeconds; processIds=@($ids);
    meanWorkingSetMB=[math]::Round(($samples | Measure-Object workingSetMB -Average).Average,2);
    peakWorkingSetMB=($samples | Measure-Object workingSetMB -Maximum).Maximum;
    meanPrivateMB=[math]::Round(($samples | Measure-Object privateMB -Average).Average,2);
    cpuPercentAllLogicalProcessors=[math]::Round((($endCpu-$startCpu)/$sampleClock.Elapsed.TotalSeconds/[Environment]::ProcessorCount)*100,3);
    startupToUsableUI='not measured by this background benchmark'; transcriptionLatency='not measured; requires live Groq key and spoken sample'; samples=$samples
  }
  $report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $out 'performance.json')
  $report | Select-Object cpu,meanWorkingSetMB,peakWorkingSetMB,meanPrivateMB,cpuPercentAllLogicalProcessors
} finally { if (-not $proc.HasExited) { Stop-Process -Id $proc.Id } }
