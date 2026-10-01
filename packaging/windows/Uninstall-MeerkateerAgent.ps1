[CmdletBinding()]
param([switch]$Purge)

$ErrorActionPreference = 'Stop'
$taskName = 'Meerkateer Agent'
$programDirectory = Join-Path $env:ProgramFiles 'Meerkateer'
$dataDirectory = Join-Path $env:ProgramData 'Meerkateer'

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Uninstall-MeerkateerAgent.ps1 must run from an elevated PowerShell session.'
}

if (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) {
    Stop-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
    Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
}

if (Test-Path -LiteralPath $programDirectory) {
    Remove-Item -LiteralPath $programDirectory -Recurse -Force
}
if ($Purge -and (Test-Path -LiteralPath $dataDirectory)) {
    Remove-Item -LiteralPath $dataDirectory -Recurse -Force
    Write-Host 'Meerkateer agent and its credential/state were permanently removed.'
} else {
    Write-Host "Meerkateer agent removed; $dataDirectory was preserved for recovery."
}
