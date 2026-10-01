[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ControllerExe,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$sourceRoot = $PSScriptRoot
$packageSource = Join-Path $sourceRoot 'Package.wxs'
$controllerPath = (Resolve-Path -LiteralPath $ControllerExe).Path
$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $outputFullPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
    throw 'WiX is required. Install the pinned tool with: dotnet tool install --global wix --version 5.0.2'
}

& wix extension add --global WixToolset.UI.wixext/5.0.2
if ($LASTEXITCODE -ne 0) { throw 'Failed to install the pinned WiX UI extension.' }

& wix build `
    -arch x64 `
    -ext WixToolset.UI.wixext `
    -d "Version=$Version" `
    -d "ControllerExe=$controllerPath" `
    -d "SourceRoot=$sourceRoot" `
    -pdbtype none `
    -o $outputFullPath `
    $packageSource
if ($LASTEXITCODE -ne 0) { throw 'WiX failed to build the Meerkateer Controller MSI.' }

Write-Host "Built $outputFullPath"
