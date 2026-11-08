# Run on Windows test machine / Windows, using the native Rust MSVC toolchain.
param([switch]$Release, [string]$ArtifactManifest, [string]$SourceDirectory)
$ErrorActionPreference = 'Stop'
$previousEncoding = [Console]::OutputEncoding
$source = if ($SourceDirectory) { [IO.Path]::GetFullPath($SourceDirectory) } else { $PSScriptRoot }
Push-Location $source
try {
    # Select the native Rust executable rather than a Linux remote-build shell shim.
    $cargoExecutable = (Get-Command cargo.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
    $toolchain = & $cargoExecutable -vV
    if ($LASTEXITCODE -ne 0 -or !($toolchain -match 'host: .*windows-msvc')) {
        throw 'A native Windows MSVC Rust toolchain is required on Windows test machine.'
    }
    $cargoArgs = @('build', '--locked')
    if ($Release) { $cargoArgs += '--release' }
    if ($ArtifactManifest) {
        [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
        $cargoArgs += '--message-format=json-render-diagnostics'
        & $cargoExecutable @cargoArgs | Tee-Object -FilePath $ArtifactManifest
    } else {
        & $cargoExecutable @cargoArgs
    }
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed ($LASTEXITCODE)" }
} finally { [Console]::OutputEncoding = $previousEncoding; Pop-Location }
