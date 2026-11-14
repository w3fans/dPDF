# Package an attested executable without compiling it again.
param(
    [Parameter(Mandatory=$true)][string]$BinaryPath,
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$WorkDirectory,
    [switch]$RequireAcceptance
)
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot)
Push-Location $root
try {
    $python = if ($env:DPDF_CI_PYTHON) { $env:DPDF_CI_PYTHON } else { (Get-Command python.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source }
    $provenancePath = Join-Path $WorkDirectory 'dpdf-native-build/build-provenance.json'
    $provenance = Get-Content -Raw $provenancePath | ConvertFrom-Json
    $commit = git rev-parse HEAD
    if ($LASTEXITCODE -ne 0 -or $provenance.identity.source_commit -ne $commit) { throw 'Build/source identity mismatch' }
    if ((Get-FileHash -Algorithm SHA256 $BinaryPath).Hash.ToLowerInvariant() -ne $provenance.binary_sha256) { throw 'Build artifact changed' }
    $sourceVersion = & $python -c "import tomllib;print(tomllib.load(open('Cargo.toml','rb'))['package']['version'])"
    if ($LASTEXITCODE -ne 0 -or $sourceVersion -ne $Version) { throw 'Package/source version mismatch' }
    if ($RequireAcceptance) {
        $accepted = $provenance.native_acceptance
        if (!$accepted -or $accepted.status -ne 'PASS' -or $accepted.source_commit -ne $commit -or $accepted.binary_sha256 -ne $provenance.binary_sha256) {
            throw 'PENDING: exact controlled native acceptance is required before private packaging'
        }
    }
    $runtime = Join-Path $WorkDirectory 'dpdf-runtime'
    & $python -c "import sys; sys.path.insert(0, 'scripts/ci'); import runtime; runtime.prepare(sys.argv[1])" $runtime
    if ($LASTEXITCODE -ne 0) { throw 'Pinned runtime preparation failed' }
    $paths = Get-Content (Join-Path $runtime 'runtimes.json') | ConvertFrom-Json
    $pdfium = Split-Path (Split-Path $paths.pdfium)
    $qpdf = Split-Path $paths.qpdf
    $stage = Join-Path $WorkDirectory 'dpdf-stage'
    & (Join-Path $PSScriptRoot 'stage.ps1') -OutputDirectory $stage -BinaryPath $BinaryPath -PdfiumDistribution $pdfium -QpdfBin $qpdf
    $out = Join-Path $WorkDirectory 'dpdf-dist'
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    Copy-Item $provenancePath $out
    $msi = Join-Path $out "dPDF-$Version-windows-x64.msi"
    & (Join-Path $PSScriptRoot 'build-msi.ps1') -StageDirectory $stage -Version $Version -Output $msi
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath (Join-Path $out "dPDF-$Version-windows-x64-portable.zip")
    $test = Join-Path $WorkDirectory 'dpdf-msi-test'
    New-Item -ItemType Directory -Force -Path $test | Out-Null
    $fixture = Join-Path $test 'fixture.pdf'
    & $python -c "import importlib.util,sys; s=importlib.util.spec_from_file_location('m','scripts/make-sample.py'); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); open(sys.argv[1],'wb').write(m.make_pdf(50))" $fixture
    if ($LASTEXITCODE -ne 0) { throw 'Controlled fixture generation failed' }
    & (Join-Path $PSScriptRoot 'test-msi.ps1') -Msi $msi -Version $Version -Fixture $fixture -WorkDirectory $test -SeedBinary (Join-Path $stage 'dpdf.exe') -SeedImage (Join-Path $root 'scripts\fixtures\page-photo.jpg')
    Get-ChildItem $out -File | Where-Object { $_.Name -ne 'SHA256SUMS.txt' } | Sort-Object Name | ForEach-Object {
        "$((Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLowerInvariant())  $($_.Name)"
    } | Set-Content -Encoding ascii (Join-Path $out 'SHA256SUMS.txt')
    if ($env:GITHUB_ENV) { "DPDF_DIST=$out" >> $env:GITHUB_ENV }
    Write-Output "Packaged preview: $out"
} finally { Pop-Location }
