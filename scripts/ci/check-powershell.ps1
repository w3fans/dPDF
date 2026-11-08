# Parse only: never execute the runner installer during a validation job.
param([Parameter(Mandatory=$true)][string]$Source)
$ErrorActionPreference = 'Stop'
$files = @((Join-Path $Source 'build.ps1'), (Join-Path $Source 'packaging/windows/stage.ps1'), (Join-Path $Source 'packaging/windows/register.ps1'),
           (Join-Path $PSScriptRoot 'install-windows-runner.ps1'), (Join-Path $Source 'scripts/windows-print-dialog.ps1'),
           (Join-Path $Source 'scripts/windows-drop-source.ps1'), (Join-Path $Source 'packaging/windows/build-msi.ps1'),
           (Join-Path $Source 'packaging/windows/test-msi.ps1'), (Join-Path $Source 'packaging/windows/package-preview.ps1'))
foreach ($file in $files) {
    $tokens = $null
    $errors = $null
    $null = [Management.Automation.Language.Parser]::ParseFile([IO.Path]::GetFullPath($file), [ref]$tokens, [ref]$errors)
    if ($errors.Count -gt 0) { throw ($errors | Out-String) }
    Write-Output "PASS: PowerShell syntax $file"
}
