# Register an already staged runtime for the current user; never change PDF defaults.
param(
    [string]$RuntimeDirectory = $PSScriptRoot,
    [string]$ApplicationId = 'net.dpdf.dPDF',
    [string]$ExecutableName = 'dpdf.exe',
    [string]$StartMenuDirectory,
    [switch]$Unregister
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'This helper requires Windows' }
if ($ApplicationId -notmatch '^net\.dpdf\.dPDF(?:\.V\.[a-f0-9]{22})?$') { throw 'Unsupported registration namespace' }
if ($ExecutableName -notmatch '^[a-zA-Z0-9._-]+\.exe$') { throw 'Choose a plain executable filename' }
$runtime = [IO.Path]::GetFullPath($RuntimeDirectory)
if ($runtime.Contains('%') -or $runtime.StartsWith('\\?\') -or $runtime.StartsWith('\\.\')) {
    throw 'Use a regular drive or UNC runtime path without percent signs'
}
$exe = Join-Path $runtime $ExecutableName
$icon = Join-Path $runtime 'dpdf.ico'
if (!$StartMenuDirectory) { $StartMenuDirectory = [Environment]::GetFolderPath('Programs') }
if (!$StartMenuDirectory) { throw 'The current user has no Start Menu Programs directory' }
$linkName = if ($ApplicationId -eq 'net.dpdf.dPDF') { 'dPDF.lnk' } else { "$ApplicationId.lnk" }
$link = Join-Path ([IO.Path]::GetFullPath($StartMenuDirectory)) $linkName
$classes = 'Software\Classes'
$owned = @("$classes\$ApplicationId", "$classes\Applications\$ExecutableName",
           "Software\Microsoft\Windows\CurrentVersion\App Paths\$ExecutableName", "Software\$ApplicationId")
function Read-Value([string]$Path, [string]$Name) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($Path)
    if ($null -eq $key) { return $null }
    try { return $key.GetValue($Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) }
    finally { $key.Dispose() }
}
function Write-Value([string]$Path, [string]$Name, [string]$Value) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($Path)
    try { $key.SetValue($Name, $Value, [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $key.Dispose() }
}
# Refuse collisions before changing any registration. Updating our previous staged
# runtime is allowed; cleanup refuses to remove another runtime's registration.
$hasRegistration = $false
foreach ($path in $owned) {
    $existing = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($path)
    if ($null -ne $existing) {
        $hasRegistration = $true
        $existing.Dispose()
        if ((Read-Value $path 'dPDF.Owner') -ne $ApplicationId) { throw "Registration already belongs to another installation: $path" }
        if ($Unregister -and (Read-Value $path 'dPDF.Executable') -ne $exe) { throw 'Run cleanup for the currently registered runtime' }
    }
}
if ($Unregister -and !$hasRegistration) {
    Write-Host 'No owned dPDF registration exists for this runtime.'
    return
}
if (!$Unregister) {
    foreach ($file in @($exe, $icon, (Join-Path $runtime 'pdfium.dll'), (Join-Path $runtime 'LICENSE'), (Join-Path $runtime 'NOTICE'))) {
        if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "Stage the complete runtime before registration; missing: $file" }
    }
}
$shell = New-Object -ComObject WScript.Shell
try {
    if (Test-Path -LiteralPath $link) {
        $shortcut = $shell.CreateShortcut($link)
        try {
            if ($shortcut.TargetPath -ne $exe -and $Unregister) { throw 'The Start Menu shortcut belongs to another runtime' }
            if ($shortcut.TargetPath -ne $exe -and (Read-Value "Software\$ApplicationId" 'dPDF.Owner') -ne $ApplicationId) { throw 'Refusing to replace an unrelated Start Menu shortcut' }
        } finally { [Runtime.InteropServices.Marshal]::ReleaseComObject($shortcut) | Out-Null }
    }
    if ($Unregister) {
        foreach ($path in $owned) { [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($path, $false) }
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("$classes\.pdf\OpenWithProgids", $true)
        if ($null -ne $key) { try { $key.DeleteValue($ApplicationId, $false) } finally { $key.Dispose() } }
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\RegisteredApplications', $true)
        if ($null -ne $key) { try { $key.DeleteValue($ApplicationId, $false) } finally { $key.Dispose() } }
        if (Test-Path -LiteralPath $link) { Remove-Item -LiteralPath $link }
    } else {
        $command = '"' + $exe + '" "%1"'
        $iconLocation = '"' + $icon + '",0'
        foreach ($path in $owned) {
            Write-Value $path 'dPDF.Owner' $ApplicationId
            Write-Value $path 'dPDF.Executable' $exe
        }
        Write-Value "$classes\$ApplicationId" '' 'dPDF PDF Document'
        Write-Value "$classes\$ApplicationId\DefaultIcon" '' $iconLocation
        Write-Value "$classes\$ApplicationId\shell\open" 'MultiSelectModel' 'Document'
        Write-Value "$classes\$ApplicationId\shell\open\command" '' $command
        Write-Value "$classes\Applications\$ExecutableName" 'FriendlyAppName' 'dPDF'
        Write-Value "$classes\Applications\$ExecutableName\SupportedTypes" '.pdf' ''
        Write-Value "$classes\Applications\$ExecutableName\DefaultIcon" '' $iconLocation
        Write-Value "$classes\Applications\$ExecutableName\shell\open\command" '' $command
        Write-Value "Software\Microsoft\Windows\CurrentVersion\App Paths\$ExecutableName" '' $exe
        Write-Value "Software\$ApplicationId\Capabilities" 'ApplicationName' 'dPDF'
        Write-Value "Software\$ApplicationId\Capabilities" 'ApplicationDescription' 'Compact PDF workspace'
        Write-Value "Software\$ApplicationId\Capabilities" 'ApplicationIcon' $iconLocation
        Write-Value "Software\$ApplicationId\Capabilities\FileAssociations" '.pdf' $ApplicationId
        Write-Value "Software\$ApplicationId\Capabilities\MimeAssociations" 'application/pdf' $ApplicationId
        Write-Value 'Software\RegisteredApplications' $ApplicationId "Software\$ApplicationId\Capabilities"
        Write-Value "$classes\.pdf\OpenWithProgids" $ApplicationId ''
        [IO.Directory]::CreateDirectory($StartMenuDirectory) | Out-Null
        $shortcut = $shell.CreateShortcut($link)
        try {
            $shortcut.TargetPath = $exe
            $shortcut.WorkingDirectory = $runtime
            $shortcut.Description = 'Compact PDF workspace'
            $shortcut.IconLocation = $iconLocation
            $shortcut.Save()
        } finally { [Runtime.InteropServices.Marshal]::ReleaseComObject($shortcut) | Out-Null }
    }
} finally { [Runtime.InteropServices.Marshal]::ReleaseComObject($shell) | Out-Null }
if (!('DPDFDesktopRegistration' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class DPDFDesktopRegistration {
    [DllImport("shell32.dll")] public static extern void SHChangeNotify(uint eventId, uint flags, IntPtr item1, IntPtr item2);
}
'@
}
[DPDFDesktopRegistration]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
if ($Unregister) { Write-Host 'Removed this dPDF user registration; PDF default settings were not changed.' }
else { Write-Host 'Registered dPDF in Start Menu / Open With for this user; PDF default settings were not changed.' }
