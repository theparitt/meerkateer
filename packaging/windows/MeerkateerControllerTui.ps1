#Requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $powerShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $arguments = '-NoProfile -ExecutionPolicy Bypass -File "{0}"' -f $PSCommandPath
    Start-Process -FilePath $powerShell -Verb RunAs -ArgumentList $arguments | Out-Null
    exit
}

$controller = Join-Path $PSScriptRoot 'meerkateer-controller.exe'
$configPath = Join-Path $env:ProgramData 'Meerkateer\agent.json'

if (-not (Test-Path -LiteralPath $controller)) {
    throw "Controller executable is missing: $controller"
}
if (-not (Test-Path -LiteralPath $configPath)) {
    Write-Host 'This computer is not connected yet.' -ForegroundColor Yellow
    Write-Host 'Open Meerkateer Controller Setup from the Start menu first.'
    [void](Read-Host 'Press Enter to close')
    exit 1
}

& $controller --config $configPath tui
if ($LASTEXITCODE -ne 0) {
    Write-Host "The local Controller dashboard exited with code $LASTEXITCODE." -ForegroundColor Red
    [void](Read-Host 'Press Enter to close')
    exit $LASTEXITCODE
}
