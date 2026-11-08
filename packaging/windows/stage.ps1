param(
    [string]$OutputDirectory = 'C:\app\dPDF',
    [Parameter(Mandatory=$true)][string]$PdfiumDistribution,
    [switch]$DebugBuild,
    [string]$BinaryPath,
    # Optional qpdf MSVC bin directory: qpdf.exe, qpdf30.dll and its app-local
    # Microsoft C++ runtime DLLs, which dpdf.exe also uses. dPDF finds qpdf.exe
    # beside its own executable.
    [string]$QpdfBin
)
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot)
$profile = if ($DebugBuild) { 'debug' } else { 'release' }
if (!$BinaryPath) {
    $targetDirectory = $env:CARGO_TARGET_DIR
    if (!$targetDirectory) { $targetDirectory = Join-Path $root 'target' }
    elseif (![IO.Path]::IsPathRooted($targetDirectory)) { $targetDirectory = Join-Path $root $targetDirectory }
    $BinaryPath = Join-Path $targetDirectory "$profile\dpdf.exe"
}
$binary = [IO.Path]::GetFullPath($BinaryPath)
$library = Join-Path $PdfiumDistribution 'bin\pdfium.dll'
foreach ($file in @($binary, $library, (Join-Path $PdfiumDistribution 'LICENSE'))) {
    if (!(Test-Path $file)) { throw "Missing runtime input: $file" }
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
Copy-Item $binary $OutputDirectory
Copy-Item $library $OutputDirectory
Copy-Item (Join-Path $PSScriptRoot 'register.ps1') $OutputDirectory
Copy-Item (Join-Path $root 'ui\assets\dpdf.ico') $OutputDirectory
$licenses = Join-Path $OutputDirectory 'licenses\pdfium'
New-Item -ItemType Directory -Force -Path $licenses | Out-Null
Copy-Item (Join-Path $PdfiumDistribution 'LICENSE') $licenses
Copy-Item (Join-Path $PdfiumDistribution 'licenses\*') $licenses -Recurse -Force
if ($QpdfBin) {
    $qpdf = Join-Path $QpdfBin 'qpdf.exe'
    if (!(Test-Path -LiteralPath $qpdf -PathType Leaf)) { throw "Missing qpdf runtime input: $qpdf" }
    Copy-Item -LiteralPath $qpdf $OutputDirectory
    Get-ChildItem -LiteralPath $QpdfBin -Filter '*.dll' | Copy-Item -Destination $OutputDirectory
    $qpdfLicenses = Join-Path $OutputDirectory 'licenses\qpdf'
    New-Item -ItemType Directory -Force -Path $qpdfLicenses | Out-Null
    Copy-Item (Join-Path $root 'packaging\licenses\qpdf\*') $qpdfLicenses -Force
}
Copy-Item (Join-Path $root 'README.md') $OutputDirectory
Copy-Item (Join-Path $root 'LICENSE') $OutputDirectory
Copy-Item (Join-Path $root 'NOTICE') $OutputDirectory
$slintLicenses = Join-Path $OutputDirectory 'licenses\slint'
New-Item -ItemType Directory -Force -Path $slintLicenses | Out-Null
Copy-Item (Join-Path $root 'packaging\licenses\slint\*') $slintLicenses -Recurse -Force
$rustLicenses = Join-Path $OutputDirectory 'licenses\rust'
New-Item -ItemType Directory -Force -Path $rustLicenses | Out-Null
Copy-Item (Join-Path $root 'packaging\licenses\rust\texts') $rustLicenses -Recurse -Force
Copy-Item (Join-Path $root 'packaging\licenses\rust\inventory-windows.json') (Join-Path $rustLicenses 'inventory.json')
Copy-Item (Join-Path $root 'packaging\licenses\rust\README.md') $rustLicenses
$docs = Join-Path $OutputDirectory 'docs'
New-Item -ItemType Directory -Force -Path $docs | Out-Null
Copy-Item (Join-Path $root 'docs\*') $docs -Recurse -Force
Write-Host "Staged development runtime: $OutputDirectory"
