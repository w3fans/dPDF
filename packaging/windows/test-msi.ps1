# Installer smoke test for CI or a disposable Windows account: per-user silent
# install, run the installed executable, check registration, uninstall, check
# cleanup. It refuses to run when dPDF is already registered for this user.
param(
    [Parameter(Mandatory=$true)][string]$Msi,
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$Fixture,
    [Parameter(Mandatory=$true)][string]$WorkDirectory,
    [string]$PreviousMsi,
    [string]$PreviousVersion
)
$ErrorActionPreference = 'Stop'
if ([bool]$PreviousMsi -ne [bool]$PreviousVersion) { throw 'Supply PreviousMsi and PreviousVersion together' }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Version must be MAJOR.MINOR.PATCH' }
if ($PreviousMsi) {
    if ($PreviousVersion -notmatch '^\d+\.\d+\.\d+$' -or [version]$PreviousVersion -ge [version]$Version) {
        throw 'PreviousVersion must be a strictly older MAJOR.MINOR.PATCH version'
    }
    $previous = [IO.Path]::GetFullPath($PreviousMsi)
    if (!(Test-Path -LiteralPath $previous -PathType Leaf)) { throw 'Previous MSI is missing' }
}
$msi = [IO.Path]::GetFullPath($Msi)
if (!(Test-Path -LiteralPath $msi -PathType Leaf)) { throw 'Candidate MSI is missing' }
Write-Host ('Candidate MSI SHA256: ' + (Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash)
if ($PreviousMsi) { Write-Host ('Previous MSI SHA256: ' + (Get-FileHash -LiteralPath $previous -Algorithm SHA256).Hash) }
$work = [IO.Path]::GetFullPath($WorkDirectory)
New-Item -ItemType Directory -Force -Path $work | Out-Null
$classes = 'HKCU:\Software\Classes\net.dpdf.dPDF'
if (Test-Path $classes) { throw 'dPDF is already registered for this user; use a disposable account' }
$install = Join-Path $env:LOCALAPPDATA 'Programs\dPDF'
$exe = Join-Path $install 'dpdf.exe'
$link = Join-Path ([Environment]::GetFolderPath('Programs')) 'dPDF.lnk'
if ((Test-Path $install) -or (Test-Path $link)) { throw 'Existing dPDF files or shortcut; use a disposable account' }

function Invoke-Msi([string[]]$Arguments, [string]$Log) {
    $process = Start-Process msiexec.exe -ArgumentList ($Arguments + @('/qn', '/norestart', '/l*v', "`"$Log`"")) -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "msiexec $($Arguments[0]) failed with $($process.ExitCode); see $Log" }
}
function Assert([bool]$Condition, [string]$Message) { if (!$Condition) { throw $Message } }

# Snapshot both legacy per-user association and Explorer's protected choice.
# Read values directly: a missing value and an explicitly empty value differ.
# Keep this state in memory; the UserChoice hash need not appear in public logs.
function Get-PdfDefaultState {
    $state = [ordered]@{}
    foreach ($path in @(
        'HKCU:\Software\Classes\.pdf',
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.pdf\UserChoice'
    )) {
        $values = [ordered]@{}
        if (Test-Path -LiteralPath $path) {
            $key = Get-Item -LiteralPath $path
            try {
                $names = @($key.GetValueNames())
                foreach ($name in @('', 'ProgId', 'Hash')) {
                    $present = $names -contains $name
                    $values[$name] = [ordered]@{
                        present = $present
                        value = $(if ($present) { $key.GetValue($name) } else { $null })
                    }
                }
            } finally { $key.Close() }
        } else {
            foreach ($name in @('', 'ProgId', 'Hash')) {
                $values[$name] = [ordered]@{ present = $false; value = $null }
            }
        }
        # Creating an otherwise empty .pdf key for Open With is permissible;
        # only the default/choice values must remain identical.
        $state[$path] = $values
    }
    return ($state | ConvertTo-Json -Depth 5 -Compress)
}
$defaultBefore = Get-PdfDefaultState

# Exercise installer retention at the application's real Windows data locations.
# Existing data is never used as a fixture or removed by this script.
if (!$env:APPDATA -or !$env:LOCALAPPDATA) { throw 'Windows application-data paths are required' }
$dataRoots = @((Join-Path $env:APPDATA 'dPDF'), (Join-Path $env:LOCALAPPDATA 'dPDF')) | Select-Object -Unique
foreach ($root in $dataRoots) {
    if (Test-Path -LiteralPath $root) { throw 'Existing dPDF user data; use a fresh disposable account' }
}
$retained = @{}
function Add-RetentionFixture([string]$Path, [string]$Content) {
    New-Item -ItemType Directory -Force -Path (Split-Path $Path) | Out-Null
    [IO.File]::WriteAllText($Path, $Content)
    $retained[$Path] = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}
function Assert-Retention {
    foreach ($path in $retained.Keys) {
        Assert (Test-Path -LiteralPath $path -PathType Leaf) 'Installer removed a generated user-data fixture'
        Assert (((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash) -ceq $retained[$path]) 'Installer changed generated user-data bytes'
    }
}
Add-RetentionFixture (Join-Path $env:APPDATA 'dPDF\preferences.json') '{"schema_version":1,"dark_theme":true,"recovery_enabled":false}'
# These are retention sentinels, not valid mark/recovery schemas. Reopening actual
# saved assets/drafts is covered separately by the native application corpora.
Add-RetentionFixture (Join-Path $env:LOCALAPPDATA 'dPDF\marks\installer-retention.txt') 'generated mark-library retention sentinel'
Add-RetentionFixture (Join-Path $env:LOCALAPPDATA 'dPDF\recovery\installer-retention.txt') 'generated recovery retention sentinel'

if ($PreviousMsi) {
    Invoke-Msi @('/i', "`"$previous`"") (Join-Path $work 'previous-install.log')
    $previousLine = (& $exe --version | Out-String).Trim()
    Assert ($LASTEXITCODE -eq 0 -and $previousLine.StartsWith("dPDF $PreviousVersion ")) 'Previous installed version mismatch'
    Assert ((Get-PdfDefaultState) -ceq $defaultBefore) 'Previous install changed the PDF default'
    Assert-Retention
}
# On failed installation retain MSI state/logs for diagnosis rather than trying
# another installer or deleting the generated data. Use a fresh disposable account.

Invoke-Msi @('/i', "`"$msi`"") (Join-Path $work 'install.log')
try {
    foreach ($file in @('dpdf.exe', 'pdfium.dll', 'qpdf.exe', 'qpdf30.dll', 'dpdf.ico', 'LICENSE', 'NOTICE', 'licenses\qpdf\NOTICE.md')) {
        Assert (Test-Path -LiteralPath (Join-Path $install $file) -PathType Leaf) "Installed file missing: $file"
    }
    Assert (!(Test-Path -LiteralPath (Join-Path $install 'register.ps1'))) 'The MSI must not ship the portable registration helper'
    # Run from an unrelated folder with no dPDF/PDFium/qpdf environment overrides.
    $env:DPDF_PDFIUM_LIBRARY = $null; $env:DPDF_QPDF_BINARY = $null
    Push-Location $work
    try {
        $versionLine = (& $exe --version | Out-String).Trim()
        Assert ($LASTEXITCODE -eq 0 -and $versionLine.StartsWith("dPDF $Version ")) "Installed version mismatch: $versionLine"
        $probe = (& $exe --probe $Fixture 2>&1 | Out-String)
        Assert ($LASTEXITCODE -eq 0 -and $probe -match 'pages=50 ') "Installed PDFium probe failed: $probe"
        $extracted = Join-Path $work 'extracted.pdf'
        $assembly = (& $exe --assemble extract $Fixture $extracted '1-3' '' 2>&1 | Out-String)
        Assert ($LASTEXITCODE -eq 0 -and (Test-Path -LiteralPath $extracted)) "Installed qpdf assembly failed: $assembly"
        $check = (& $exe --probe $extracted 2>&1 | Out-String)
        Assert ($check -match 'pages=3 ') "Extracted copy has the wrong page count: $check"
    } finally { Pop-Location }
    $command = (Get-ItemProperty "$classes\shell\open\command").'(default)'
    Assert ($command -eq "`"$exe`" `"%1`"") "Unexpected open command: $command"
    Assert ((Get-ItemProperty 'HKCU:\Software\Classes\.pdf\OpenWithProgids').PSObject.Properties.Name -contains 'net.dpdf.dPDF') 'Open With entry missing'
    Assert ((Get-ItemProperty 'HKCU:\Software\RegisteredApplications').'net.dpdf.dPDF' -eq 'Software\net.dpdf.dPDF\Capabilities') 'Registered application missing'
    Assert ((Get-PdfDefaultState) -ceq $defaultBefore) 'Install changed the existing PDF default association or UserChoice'
    Assert (Test-Path -LiteralPath $link) 'Start Menu shortcut missing'
    Assert-Retention
    if ($PreviousMsi) { Write-Host 'PASS: older installed version upgraded to candidate with generated user-data bytes intact' }
    Write-Host 'PASS: per-user MSI install, launch, PDFium probe, bundled qpdf assembly, Open With registration, Start Menu shortcut; .pdf default unchanged'
} finally {
    Invoke-Msi @('/x', "`"$msi`"") (Join-Path $work 'uninstall.log')
}
Assert (!(Test-Path -LiteralPath $exe)) 'Uninstall left dpdf.exe behind'
Assert (!(Test-Path $classes)) 'Uninstall left the ProgID registration'
Assert (!(Test-Path -LiteralPath $link)) 'Uninstall left the Start Menu shortcut'
$openWith = Get-ItemProperty 'HKCU:\Software\Classes\.pdf\OpenWithProgids' -ErrorAction SilentlyContinue
Assert (!($openWith -and $openWith.PSObject.Properties.Name -contains 'net.dpdf.dPDF')) 'Uninstall left the Open With entry'
Assert ((Get-PdfDefaultState) -ceq $defaultBefore) 'Uninstall changed the existing PDF default association or UserChoice'
Write-Host 'PASS: uninstall removed files, registration and shortcut'

Assert-Retention
Write-Host 'PASS: install/uninstall retained generated preferences, mark-library and recovery sentinel bytes'
# Success-only cleanup touches roots that were absent before this fixture.
foreach ($root in $dataRoots) { Remove-Item -LiteralPath $root -Recurse -Force }
