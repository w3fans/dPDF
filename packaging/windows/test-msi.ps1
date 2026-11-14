# Installer smoke test for CI or a disposable Windows account: per-user silent
# install, run the installed executable, check registration, uninstall, check
# cleanup. It refuses to run when dPDF is already registered for this user.
param(
    [Parameter(Mandatory=$true)][string]$Msi,
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$Fixture,
    [Parameter(Mandatory=$true)][string]$WorkDirectory,
    [string]$PreviousMsi,
    [string]$PreviousVersion,
    [string]$SeedBinary,
    [string]$SeedImage
)
$ErrorActionPreference = 'Stop'
$seedBinaryPath = $null
if ([bool]$PreviousMsi -ne [bool]$PreviousVersion) { throw 'Supply PreviousMsi and PreviousVersion together' }
if ([bool]$SeedBinary -ne [bool]$SeedImage) { throw 'Supply SeedBinary and SeedImage together' }
if ($SeedBinary) {
    $seedBinaryPath = [IO.Path]::GetFullPath($SeedBinary)
    $seedImagePath = [IO.Path]::GetFullPath($SeedImage)
    foreach ($inputPath in @($seedBinaryPath, $seedImagePath)) {
        if (!(Test-Path -LiteralPath $inputPath -PathType Leaf)) { throw 'Mark fixture input is missing' }
    }
    $seedBinaryHash = (Get-FileHash -LiteralPath $seedBinaryPath -Algorithm SHA256).Hash
    $seedImageHash = (Get-FileHash -LiteralPath $seedImagePath -Algorithm SHA256).Hash
}
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
# Sentinels cover byte retention. The optional fixture below additionally reloads
# a valid mark model; recovery drafts and GUI behavior remain separate corpora.
Add-RetentionFixture (Join-Path $env:LOCALAPPDATA 'dPDF\marks\installer-retention.txt') 'generated mark-library retention sentinel'
Add-RetentionFixture (Join-Path $env:LOCALAPPDATA 'dPDF\recovery\installer-retention.txt') 'generated recovery retention sentinel'

$expectedMarkList = $null
function Get-MarkListState([string]$Binary) {
    Assert (((Get-FileHash -LiteralPath $Binary -Algorithm SHA256).Hash) -ceq $seedBinaryHash) 'Mark reload binary differs from the candidate seed executable'
    $output = (& $Binary --marks list | Out-String)
    Assert ($LASTEXITCODE -eq 0) 'Candidate could not reload the generated mark library'
    $assets = @($output | ConvertFrom-Json)
    Assert ($assets.Count -eq 1) 'Expected exactly one generated mark asset'
    Assert ($assets[0].schema_version -eq 1 -and $assets[0].id -ceq $seedAsset.id -and $assets[0].name -ceq 'Installer retention fixture' -and $assets[0].kind -ceq 'stamp') 'Generated mark metadata did not reload'
    return (ConvertTo-Json -InputObject $assets -Depth 12 -Compress)
}
function Assert-MarkReload([string]$Binary) {
    if ($SeedBinary) {
        Assert ((Get-MarkListState $Binary) -ceq $expectedMarkList) 'Installer changed the reloaded mark model'
        Assert-Retention
    }
}
if ($SeedBinary) {
    # Use the real data roots already proven absent above. CLI overrides must
    # not redirect this check to a separate library from the installed app.
    $env:DPDF_DATA_DIR = $null; $env:DPDF_CONFIG_DIR = $null
    $seedVersion = (& $seedBinaryPath --version | Out-String).Trim()
    Assert ($LASTEXITCODE -eq 0 -and $seedVersion.StartsWith("dPDF $Version ")) 'Seed binary version differs from candidate'
    $library = Join-Path $env:LOCALAPPDATA 'dPDF\marks'
    $imported = (& $seedBinaryPath --marks import $seedImagePath 'Installer retention fixture' stamp $library | Out-String)
    Assert ($LASTEXITCODE -eq 0) 'Cannot seed the generated mark model'
    $seedAsset = $imported | ConvertFrom-Json
    Assert ($seedAsset.id -match '^asset-[A-Za-z0-9_-]+$') 'Invalid generated mark fixture identity'
    $assetFolder = Join-Path $library $seedAsset.id
    $markHashes = [ordered]@{}
    foreach ($name in @('metadata.json', 'original', 'cleaned.png', 'thumbnail.png')) {
        $path = Join-Path $assetFolder $name
        Assert (Test-Path -LiteralPath $path -PathType Leaf) 'Generated mark model file is missing'
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
        $retained[$path] = $hash
        $markHashes[$name] = $hash.ToLowerInvariant()
    }
    Assert ($markHashes['original'] -ceq $seedImageHash.ToLowerInvariant()) 'Seeded original differs from the generated source image'
    foreach ($name in @('cleaned.png', 'thumbnail.png')) {
        $decoded = Join-Path $work ('decoded-' + $name)
        $decodeOutput = (& $seedBinaryPath --marks clean (Join-Path $assetFolder $name) $decoded | Out-String)
        Assert ($LASTEXITCODE -eq 0 -and (Test-Path -LiteralPath $decoded -PathType Leaf)) 'Generated mark PNG could not be decoded'
    }
    $expectedMarkList = Get-MarkListState $seedBinaryPath
    Assert-Retention
}

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
    Assert-MarkReload $exe
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
Assert-MarkReload $seedBinaryPath
if ($SeedBinary) {
    Assert (((Get-FileHash -LiteralPath $seedImagePath -Algorithm SHA256).Hash) -ceq $seedImageHash) 'Mark import changed the generated source image'
    $receipt = [ordered]@{
        schema = 1
        scope = 'generated mark library CLI reload; not recovery, GUI or old-format migration acceptance'
        candidate_version = $Version
        binary_sha256 = $seedBinaryHash.ToLowerInvariant()
        seed_image_sha256 = $seedImageHash.ToLowerInvariant()
        asset_id = $seedAsset.id
        retained_files = $markHashes
        installed_and_post_uninstall_reload = $true
    }
    Write-Host ('MSI mark-retention receipt: ' + ($receipt | ConvertTo-Json -Depth 6 -Compress))
    Write-Host 'PASS: installed candidate and retained staged executable reload the unchanged generated mark library'
}
Write-Host 'PASS: install/uninstall retained generated preferences, mark-library and recovery sentinel bytes'
# Success-only cleanup touches roots that were absent before this fixture.
foreach ($root in $dataRoots) { Remove-Item -LiteralPath $root -Recurse -Force }
