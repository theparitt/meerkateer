#Requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $powerShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $arguments = '-NoProfile -ExecutionPolicy Bypass -File "{0}"' -f $PSCommandPath
    Start-Process -FilePath $powerShell -Verb RunAs -ArgumentList $arguments | Out-Null
    exit
}

$controller = Join-Path $PSScriptRoot 'meerkateer-controller.exe'
$installer = Join-Path $PSScriptRoot 'Install-MeerkateerAgent.ps1'
$dataDirectory = Join-Path $env:ProgramData 'Meerkateer'
$configPath = Join-Path $dataDirectory 'agent.json'
$disclaimerPath = Join-Path $PSScriptRoot 'DISCLAIMER.md'

if (-not (Test-Path -LiteralPath $controller)) {
    [System.Windows.Forms.MessageBox]::Show(
        "Controller executable is missing: $controller",
        'Meerkateer Controller',
        'OK',
        'Error'
    ) | Out-Null
    exit 1
}

function Add-FieldLabel {
    param([string]$Text, [int]$Top)
    $label = [System.Windows.Forms.Label]::new()
    $label.Text = $Text
    $label.Location = [Drawing.Point]::new(28, $Top)
    $label.Size = [Drawing.Size]::new(540, 22)
    $label.Font = [Drawing.Font]::new('Segoe UI', 9, [Drawing.FontStyle]::Bold)
    return $label
}

function Invoke-Controller {
    param(
        [string[]]$Arguments,
        [AllowEmptyString()][string]$EnrollmentToken = ''
    )
    $hadToken = Test-Path Env:MEERKATEER_ENROLLMENT_TOKEN
    $previousToken = $env:MEERKATEER_ENROLLMENT_TOKEN
    try {
        if ($EnrollmentToken) {
            $env:MEERKATEER_ENROLLMENT_TOKEN = $EnrollmentToken
        } else {
            Remove-Item Env:MEERKATEER_ENROLLMENT_TOKEN -ErrorAction SilentlyContinue
        }
        $output = & $controller @Arguments 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw (($output | Out-String).Trim())
        }
        return ($output | Out-String).Trim()
    } finally {
        if ($hadToken) {
            $env:MEERKATEER_ENROLLMENT_TOKEN = $previousToken
        } else {
            Remove-Item Env:MEERKATEER_ENROLLMENT_TOKEN -ErrorAction SilentlyContinue
        }
    }
}

function Get-SignalArguments {
    $signals = [Collections.Generic.List[string]]::new()
    if ($signalList.GetItemChecked(0)) { $signals.Add('cpu') }
    if ($signalList.GetItemChecked(1)) { $signals.Add('memory') }
    if ($signalList.GetItemChecked(2)) { $signals.Add('disk') }
    if ($signalList.GetItemChecked(3)) { $signals.Add('process') }
    if ($signals.Count -eq 0) {
        throw 'Choose at least one signal. A bounded heartbeat is always included.'
    }

    $arguments = [Collections.Generic.List[string]]::new()
    $arguments.Add('--config')
    $arguments.Add($configPath)
    $arguments.Add('configure')
    $arguments.Add('--signals')
    $arguments.Add(($signals -join ','))

    $processNames = @()
    if ($signalList.GetItemChecked(3)) {
        $processNames = $processBox.Text.Split(',', [StringSplitOptions]::RemoveEmptyEntries) |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ }
    }
    if ($processNames.Count -gt 16) {
        throw 'At most 16 exact process names can be monitored.'
    }
    foreach ($processName in $processNames) {
        $arguments.Add('--watch-process')
        $arguments.Add($processName)
    }
    return $arguments.ToArray()
}

function Install-BackgroundController {
    & $installer -BinaryPath $controller -ConfigPath $configPath -ReplaceConfig
    if ($LASTEXITCODE -ne 0) {
        throw 'The background startup task could not be registered.'
    }
}

function Protect-EnrollmentDirectory {
    New-Item -ItemType Directory -Force -Path $dataDirectory | Out-Null
    & icacls.exe $dataDirectory /inheritance:r /grant:r `
        '*S-1-5-18:(OI)(CI)F' `
        '*S-1-5-32-544:(OI)(CI)F' | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not secure the local credential directory.'
    }
}

function Invoke-ConnectionDiagnostics {
    param([string]$ServerUrl)
    $json = Invoke-Controller -Arguments @(
        '--config', $configPath, 'test-connection', '--server', $ServerUrl
    )
    $report = $json | ConvertFrom-Json
    $lines = [Collections.Generic.List[string]]::new()
    foreach ($check in @($report.checks)) {
        $marker = ([string]$check.state).ToUpperInvariant()
        $lines.Add(('[{0}] {1}: {2}' -f $marker, $check.code, $check.message))
        if ($check.hint -and $check.state -in @('warn', 'fail')) {
            $lines.Add(('    Next: {0}' -f $check.hint))
        }
    }
    return [pscustomobject]@{
        Ok = ([string]$report.status -eq 'ok')
        Summary = ($lines -join [Environment]::NewLine)
    }
}

$form = [System.Windows.Forms.Form]::new()
$form.Text = 'Meerkateer Controller Setup'
$form.StartPosition = 'CenterScreen'
$form.ClientSize = [Drawing.Size]::new(630, 850)
$form.MinimumSize = [Drawing.Size]::new(646, 889)
$form.BackColor = [Drawing.Color]::FromArgb(255, 250, 242)
$form.Font = [Drawing.Font]::new('Segoe UI', 9)

$title = [System.Windows.Forms.Label]::new()
$title.Text = 'Connect this computer to Meerkateer'
$title.Location = [Drawing.Point]::new(28, 22)
$title.Size = [Drawing.Size]::new(570, 38)
$title.Font = [Drawing.Font]::new('Segoe UI', 18, [Drawing.FontStyle]::Bold)
$title.ForeColor = [Drawing.Color]::FromArgb(7, 25, 82)
$form.Controls.Add($title)

$intro = [System.Windows.Forms.Label]::new()
$intro.Text = 'The controller only opens outbound HTTPS connections. Choose exactly what this machine may report.'
$intro.Location = [Drawing.Point]::new(30, 66)
$intro.Size = [Drawing.Size]::new(565, 38)
$intro.ForeColor = [Drawing.Color]::FromArgb(73, 86, 124)
$form.Controls.Add($intro)

$destinationLabel = Add-FieldLabel 'Where should this computer send reliability signals?' 112
$form.Controls.Add($destinationLabel)
$localRadio = [System.Windows.Forms.RadioButton]::new()
$localRadio.Text = 'Local / self-hosted Community'
$localRadio.Location = [Drawing.Point]::new(30, 138)
$localRadio.Size = [Drawing.Size]::new(260, 28)
$localRadio.Checked = $true
$form.Controls.Add($localRadio)
$cloudRadio = [System.Windows.Forms.RadioButton]::new()
$cloudRadio.Text = 'Meerkateer Cloud - coming soon'
$cloudRadio.Location = [Drawing.Point]::new(310, 138)
$cloudRadio.Size = [Drawing.Size]::new(285, 28)
$cloudRadio.Enabled = $false
$form.Controls.Add($cloudRadio)

$serverLabel = Add-FieldLabel 'Self-hosted Meerkateer API URL' 181
$form.Controls.Add($serverLabel)
$serverBox = [System.Windows.Forms.TextBox]::new()
$serverBox.Location = [Drawing.Point]::new(30, 205)
$serverBox.Size = [Drawing.Size]::new(405, 28)
$serverBox.Text = 'https://'
$form.Controls.Add($serverBox)

$testButton = [System.Windows.Forms.Button]::new()
$testButton.Text = 'Test connection'
$testButton.Location = [Drawing.Point]::new(445, 201)
$testButton.Size = [Drawing.Size]::new(150, 34)
$testButton.FlatStyle = 'Flat'
$form.Controls.Add($testButton)

$nameLabel = Add-FieldLabel 'Computer name shown in the workspace' 246
$form.Controls.Add($nameLabel)
$nameBox = [System.Windows.Forms.TextBox]::new()
$nameBox.Location = [Drawing.Point]::new(30, 270)
$nameBox.Size = [Drawing.Size]::new(565, 28)
$nameBox.Text = $env:COMPUTERNAME
$form.Controls.Add($nameBox)

$tokenLabel = Add-FieldLabel 'One-time enrollment token from Console > Connect' 311
$form.Controls.Add($tokenLabel)
$tokenBox = [System.Windows.Forms.TextBox]::new()
$tokenBox.Location = [Drawing.Point]::new(30, 335)
$tokenBox.Size = [Drawing.Size]::new(565, 28)
$tokenBox.UseSystemPasswordChar = $true
$form.Controls.Add($tokenBox)

$signalLabel = Add-FieldLabel 'Signals sent every 30 seconds' 380
$form.Controls.Add($signalLabel)
$signalList = [System.Windows.Forms.CheckedListBox]::new()
$signalList.Location = [Drawing.Point]::new(30, 405)
$signalList.Size = [Drawing.Size]::new(565, 112)
$signalList.CheckOnClick = $true
$signalList.Items.Add('CPU utilization - percentage only') | Out-Null
$signalList.Items.Add('Memory capacity and usage - byte counts only') | Out-Null
$signalList.Items.Add('Disk capacity and usage - totals only; no filenames') | Out-Null
$signalList.Items.Add('Selected process running state - exact names you enter below') | Out-Null
$signalList.SetItemChecked(0, $true)
$signalList.SetItemChecked(1, $true)
$signalList.SetItemChecked(2, $true)
$form.Controls.Add($signalList)

$processLabel = Add-FieldLabel 'Process names (comma-separated, maximum 16)' 531
$form.Controls.Add($processLabel)
$processBox = [System.Windows.Forms.TextBox]::new()
$processBox.Location = [Drawing.Point]::new(30, 555)
$processBox.Size = [Drawing.Size]::new(565, 28)
$processBox.Text = ''
$form.Controls.Add($processBox)

$privacy = [System.Windows.Forms.Label]::new()
$privacy.Text = 'Never collected: command-line arguments, files, environment variables, player/chat content, or arbitrary remote commands.'
$privacy.Location = [Drawing.Point]::new(30, 595)
$privacy.Size = [Drawing.Size]::new(565, 40)
$privacy.ForeColor = [Drawing.Color]::FromArgb(73, 86, 124)
$form.Controls.Add($privacy)

$previewConsent = [System.Windows.Forms.CheckBox]::new()
$previewConsent.Text = 'I am authorized to administer this computer and accept the Developer Preview notice: AS IS, unsigned, without warranty; I will verify the package checksum and protect the API.'
$previewConsent.Location = [Drawing.Point]::new(30, 638)
$previewConsent.Size = [Drawing.Size]::new(565, 58)
$previewConsent.ForeColor = [Drawing.Color]::FromArgb(83, 61, 23)
$form.Controls.Add($previewConsent)

$noticeLink = [System.Windows.Forms.LinkLabel]::new()
$noticeLink.Text = 'Read the complete Developer Preview notice'
$noticeLink.Location = [Drawing.Point]::new(50, 694)
$noticeLink.Size = [Drawing.Size]::new(360, 24)
$noticeLink.Add_LinkClicked({
    if (Test-Path -LiteralPath $disclaimerPath) {
        Start-Process -FilePath 'notepad.exe' -ArgumentList ('"{0}"' -f $disclaimerPath) | Out-Null
    } else {
        [System.Windows.Forms.MessageBox]::Show(
            'The installed DISCLAIMER.md file is missing. Reinstall from the official release before connecting this computer.',
            'Meerkateer Controller',
            'OK',
            'Error'
        ) | Out-Null
    }
})
$form.Controls.Add($noticeLink)

$connectButton = [System.Windows.Forms.Button]::new()
$connectButton.Text = 'Connect and start'
$connectButton.Location = [Drawing.Point]::new(30, 730)
$connectButton.Size = [Drawing.Size]::new(270, 42)
$connectButton.BackColor = [Drawing.Color]::FromArgb(20, 101, 183)
$connectButton.ForeColor = [Drawing.Color]::White
$connectButton.FlatStyle = 'Flat'
$connectButton.Enabled = $false
$form.Controls.Add($connectButton)

$saveButton = [System.Windows.Forms.Button]::new()
$saveButton.Text = 'Update signals only'
$saveButton.Location = [Drawing.Point]::new(325, 730)
$saveButton.Size = [Drawing.Size]::new(270, 42)
$saveButton.FlatStyle = 'Flat'
$form.Controls.Add($saveButton)

$existingConfig = $null
if (Test-Path -LiteralPath $configPath) {
    try {
        $existingConfig = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
        $serverBox.Text = [string]$existingConfig.server_url
        $nameBox.Text = [string]$existingConfig.display_name
        $serverBox.Enabled = $false
        $nameBox.Enabled = $false
        $tokenBox.Enabled = $false
        $connectButton.Enabled = $false
        $previewConsent.Checked = $true
        $previewConsent.Enabled = $false

        if ($null -ne $existingConfig.signals) {
            for ($index = 0; $index -lt $signalList.Items.Count; $index++) {
                $signalList.SetItemChecked($index, $false)
            }
            $enabledSignals = @($existingConfig.signals.enabled)
            $signalList.SetItemChecked(0, $enabledSignals -contains 'cpu')
            $signalList.SetItemChecked(1, $enabledSignals -contains 'memory')
            $signalList.SetItemChecked(2, $enabledSignals -contains 'disk')
            $signalList.SetItemChecked(3, $enabledSignals -contains 'process')
            $processBox.Text = (@($existingConfig.signals.watched_processes) -join ', ')
        }
    } catch {
        [System.Windows.Forms.MessageBox]::Show(
            "The existing controller configuration could not be read. Run the controller doctor command before changing it.`n`n$($_.Exception.Message)",
            'Meerkateer Controller',
            'OK',
            'Error'
        ) | Out-Null
        exit 1
    }
}

$status = [System.Windows.Forms.Label]::new()
$status.Text = if ($null -ne $existingConfig) {
    'Connected controller found. Host and identity are locked; signal choices can be updated.'
} else {
    'Not connected yet.'
}
$status.Location = [Drawing.Point]::new(30, 790)
$status.Size = [Drawing.Size]::new(565, 32)
$status.ForeColor = [Drawing.Color]::FromArgb(7, 113, 83)
$form.Controls.Add($status)

$previewConsent.Add_CheckedChanged({
    if ($null -eq $existingConfig) {
        $connectButton.Enabled = $previewConsent.Checked
    }
})

$testButton.Add_Click({
    try {
        $status.Text = 'Testing disk/config, DNS, network route, TLS, and API readiness...'
        $form.Refresh()
        $diagnostics = Invoke-ConnectionDiagnostics -ServerUrl $serverBox.Text.Trim()
        if ($diagnostics.Ok) {
            $status.Text = 'Connection healthy. No credential or telemetry was sent.'
            [System.Windows.Forms.MessageBox]::Show(
                $diagnostics.Summary,
                'Meerkateer connection test passed',
                'OK',
                'Information'
            ) | Out-Null
        } else {
            $status.Text = 'Connection test found a blocking problem.'
            [System.Windows.Forms.MessageBox]::Show(
                $diagnostics.Summary,
                'Meerkateer connection test',
                'OK',
                'Warning'
            ) | Out-Null
        }
    } catch {
        $status.Text = 'Connection test could not complete.'
        [System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Meerkateer connection test', 'OK', 'Error') | Out-Null
    }
})

$connectButton.Add_Click({
    try {
        if (Test-Path -LiteralPath $configPath) {
            throw 'This computer is already enrolled. Use Update signals only, or uninstall and purge before enrolling it again.'
        }
        if (-not $previewConsent.Checked) {
            throw 'Read and accept the Developer Preview notice before connecting this computer.'
        }
        if ([string]::IsNullOrWhiteSpace($tokenBox.Text)) {
            throw 'Paste the one-time enrollment token from the Meerkateer Console.'
        }
        $status.Text = 'Checking this computer and the self-hosted API...'
        $form.Refresh()
        $diagnostics = Invoke-ConnectionDiagnostics -ServerUrl $serverBox.Text.Trim()
        if (-not $diagnostics.Ok) {
            throw "Connection checks failed. Fix these before enrollment:`n`n$($diagnostics.Summary)"
        }
        Protect-EnrollmentDirectory
        $status.Text = 'Enrolling this computer...'
        $form.Refresh()
        Invoke-Controller -Arguments @(
            '--config', $configPath, 'enroll', '--destination', 'self-hosted', '--server', $serverBox.Text.Trim(), '--name', $nameBox.Text.Trim()
        ) -EnrollmentToken $tokenBox.Text | Out-Null
        $tokenBox.Clear()
        Invoke-Controller -Arguments (Get-SignalArguments) | Out-Null
        Install-BackgroundController
        Invoke-Controller -Arguments @('--config', $configPath, 'doctor') | Out-Null
        $status.Text = 'Connected. The background controller is running.'
        [System.Windows.Forms.MessageBox]::Show(
            'This computer is connected and its selected signals are now being sent.',
            'Meerkateer Controller',
            'OK',
            'Information'
        ) | Out-Null
    } catch {
        $tokenBox.Clear()
        $status.Text = 'Setup did not complete.'
        [System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Meerkateer Controller', 'OK', 'Error') | Out-Null
    }
})

$saveButton.Add_Click({
    try {
        if (-not (Test-Path -LiteralPath $configPath)) {
            throw 'Connect this computer first.'
        }
        $status.Text = 'Saving signal choices...'
        $form.Refresh()
        Invoke-Controller -Arguments (Get-SignalArguments) | Out-Null
        Install-BackgroundController
        $status.Text = 'Signal choices saved. The controller has restarted.'
    } catch {
        $status.Text = 'Signal choices were not changed.'
        [System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Meerkateer Controller', 'OK', 'Error') | Out-Null
    }
})

[void]$form.ShowDialog()
