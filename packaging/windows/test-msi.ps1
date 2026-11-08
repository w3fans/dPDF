# Installer smoke test for CI or a disposable Windows account: per-user silent
# install, run the installed executable, check registration, uninstall, check
# cleanup. It refuses to run when dPDF is already registered for this user.
param(
    [Parameter(Mandatory=$true)][string]$Msi,
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$Fixture,
    [Parameter(Mandatory=$true)][string]$WorkDirectory
)
$ErrorActionPreference = 'Stop'
$msi = [IO.Path]::GetFullPath($Msi)
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
    $default = (Get-ItemProperty 'HKCU:\Software\Classes\.pdf' -ErrorAction SilentlyContinue).'(default)'
    Assert ($default -ne 'net.dpdf.dPDF') 'The installer must not take the .pdf default'
    Assert (Test-Path -LiteralPath $link) 'Start Menu shortcut missing'
    Write-Host 'PASS: per-user MSI install, launch, PDFium probe, bundled qpdf assembly, Open With registration, Start Menu shortcut; .pdf default unchanged'
} finally {
    Invoke-Msi @('/x', "`"$msi`"") (Join-Path $work 'uninstall.log')
}
Assert (!(Test-Path -LiteralPath $exe)) 'Uninstall left dpdf.exe behind'
Assert (!(Test-Path $classes)) 'Uninstall left the ProgID registration'
Assert (!(Test-Path -LiteralPath $link)) 'Uninstall left the Start Menu shortcut'
$openWith = Get-ItemProperty 'HKCU:\Software\Classes\.pdf\OpenWithProgids' -ErrorAction SilentlyContinue
Assert (!($openWith -and $openWith.PSObject.Properties.Name -contains 'net.dpdf.dPDF')) 'Uninstall left the Open With entry'
Write-Host 'PASS: uninstall removed files, registration and shortcut'
