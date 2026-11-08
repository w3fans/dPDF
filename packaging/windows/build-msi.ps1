# Build the dPDF MSI from a runtime staged by stage.ps1 -QpdfBin.
# WiX is pinned and installed as a local tool under -ToolDirectory, never globally.
param(
    [Parameter(Mandatory=$true)][string]$StageDirectory,
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$Output,
    [string]$ToolDirectory = (Join-Path ([IO.Path]::GetTempPath()) 'dpdf-wix-5.0.2')
)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "MSI version must be MAJOR.MINOR.PATCH: $Version" }
$stage = [IO.Path]::GetFullPath($StageDirectory)
foreach ($file in @('dpdf.exe', 'pdfium.dll', 'qpdf.exe', 'qpdf30.dll', 'vcruntime140.dll', 'dpdf.ico',
                    'LICENSE', 'NOTICE', 'licenses\pdfium\LICENSE', 'licenses\qpdf\NOTICE.md')) {
    if (!(Test-Path -LiteralPath (Join-Path $stage $file) -PathType Leaf)) { throw "Incomplete staged runtime; missing $file" }
}
$wix = Join-Path $ToolDirectory 'wix.exe'
if (!(Test-Path -LiteralPath $wix)) {
    & dotnet tool install wix --version 5.0.2 --tool-path $ToolDirectory
    if ($LASTEXITCODE -ne 0) { throw "WiX installation failed ($LASTEXITCODE)" }
}
$installed = (& $wix --version).Trim()
if ($installed -notlike '5.0.2*') { throw "Unexpected WiX version: $installed" }
$source = Join-Path $PSScriptRoot 'dpdf.wxs'
$target = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
& $wix build $source -arch x64 -d "Version=$Version" -bindpath "stage=$stage" -o $target
if ($LASTEXITCODE -ne 0) { throw "WiX build failed ($LASTEXITCODE)" }
# The WiX debug database is a build by-product, not something to publish.
Remove-Item -LiteralPath ([IO.Path]::ChangeExtension($target, '.wixpdb')) -ErrorAction SilentlyContinue
Write-Host "Built $target"
