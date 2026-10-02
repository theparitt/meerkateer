[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath,

    [Parameter(Mandatory = $true)]
    [string]$ConfigPath,

    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._ -]{0,63}$')]
    [string[]]$WatchProcess = @(),

    [switch]$NoStart,
    [switch]$ReplaceConfig
)

$ErrorActionPreference = 'Stop'
$taskName = 'Meerkateer Controller'
$programDirectory = Join-Path $env:ProgramFiles 'Meerkateer'
$dataDirectory = Join-Path $env:ProgramData 'Meerkateer'
$installedBinary = Join-Path $programDirectory 'meerkateer-controller.exe'
$installedConfig = Join-Path $dataDirectory 'agent.json'

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Install-MeerkateerAgent.ps1 must run from an elevated PowerShell session.'
}

$sourceBinary = (Resolve-Path -LiteralPath $BinaryPath).Path
$sourceConfig = (Resolve-Path -LiteralPath $ConfigPath).Path
if ((Get-Item -LiteralPath $sourceBinary).PSIsContainer -or (Get-Item -LiteralPath $sourceConfig).PSIsContainer) {
    throw 'BinaryPath and ConfigPath must identify files.'
}
if ([IO.Path]::GetExtension($sourceBinary) -ne '.exe') {
    throw 'BinaryPath must identify the Meerkateer Controller executable.'
}
if ((Test-Path -LiteralPath $installedConfig) -and -not $ReplaceConfig) {
    throw "$installedConfig already exists. Use -ReplaceConfig only when rotating this machine identity."
}

& $sourceBinary --config $sourceConfig doctor | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw 'Agent doctor rejected the supplied configuration.'
}

if (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) {
    Stop-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
}

New-Item -ItemType Directory -Force -Path $programDirectory, $dataDirectory | Out-Null
if (-not [StringComparer]::OrdinalIgnoreCase.Equals($sourceBinary, $installedBinary)) {
    Copy-Item -LiteralPath $sourceBinary -Destination $installedBinary -Force
}
if (-not [StringComparer]::OrdinalIgnoreCase.Equals($sourceConfig, $installedConfig)) {
    Copy-Item -LiteralPath $sourceConfig -Destination $installedConfig -Force
}

# The machine credential is writable only by Administrators, SYSTEM, and the low-privilege task identity.
& icacls.exe $dataDirectory /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-19:(OI)(CI)M' | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Failed to secure the Meerkateer data directory ACL.' }
& icacls.exe $installedConfig /inheritance:r /grant:r '*S-1-5-18:F' '*S-1-5-32-544:F' '*S-1-5-19:M' | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Failed to secure the Meerkateer credential ACL.' }

$arguments = '--config "{0}" run' -f $installedConfig
foreach ($processName in $WatchProcess) {
    $arguments += ' --watch-process "{0}"' -f $processName
}

$action = New-ScheduledTaskAction -Execute $installedBinary -Argument $arguments
$trigger = New-ScheduledTaskTrigger -AtStartup
$taskPrincipal = New-ScheduledTaskPrincipal -UserId 'LOCALSERVICE' -LogonType ServiceAccount -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -StartWhenAvailable `
    -RestartCount 999 `
    -RestartInterval (New-TimeSpan -Minutes 1) `
    -ExecutionTimeLimit ([TimeSpan]::Zero)

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger `
    -Principal $taskPrincipal -Settings $settings -Description `
    'Starts the outbound-only Meerkateer Controller automatically when Windows starts.' `
    -Force | Out-Null

$registeredTask = Get-ScheduledTask -TaskName $taskName -ErrorAction Stop
$hasStartupTrigger = @(
    $registeredTask.Triggers | Where-Object { $_.CimClass.CimClassName -eq 'MSFT_TaskBootTrigger' }
).Count -gt 0
if (-not $hasStartupTrigger) {
    throw "Startup task '$taskName' was registered without a Windows boot trigger."
}
if ($registeredTask.State -eq 'Disabled') {
    Enable-ScheduledTask -TaskName $taskName | Out-Null
}

if (-not $NoStart) {
    Start-ScheduledTask -TaskName $taskName
    $started = $false
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        if ((Get-ScheduledTask -TaskName $taskName -ErrorAction Stop).State -eq 'Running') {
            $started = $true
            break
        }
        Start-Sleep -Milliseconds 250
    }
    if (-not $started) {
        $taskInfo = Get-ScheduledTaskInfo -TaskName $taskName -ErrorAction Stop
        throw "Startup task '$taskName' did not remain running. LastTaskResult=$($taskInfo.LastTaskResult)."
    }
}

Write-Host "Meerkateer Controller installed as startup task '$taskName'."
Write-Host 'Automatic startup: enabled (Windows boot trigger).'
Write-Host "Configuration: $installedConfig"
