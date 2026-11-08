# One-time local setup. Run from an elevated Windows PowerShell prompt on Windows test machine.
# Does not edit dGK, the user's Rust defaults, firewall rules, or desktop login.
param(
    [string]$Root = 'C:\actions-runner-dpdf',
    [switch]$Desktop
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT' -or ![Environment]::Is64BitOperatingSystem) { throw 'Requires Windows x64.' }
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (!$Desktop -and !$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Service installation requires an elevated local Windows PowerShell prompt.'
}
if ($Desktop -and $Root -eq 'C:\actions-runner-dpdf') { $Root = 'C:\actions-runner-dpdf-desktop' }
$Root = [IO.Path]::GetFullPath($Root).TrimEnd('\')
if ($Root -match '[\r\n]' -or $Root -match '(?i)\\(zDev|dGK)(\\|$)' -or $Root.Length -lt 12) {
    throw 'Choose a dedicated runner directory outside development trees.'
}
if ((Test-Path -LiteralPath $Root) -and (Get-ChildItem -Force -LiteralPath $Root | Select-Object -First 1)) {
    throw 'Runner directory must be empty. Existing runners are never replaced automatically.'
}
$expectedName = if ($Desktop) { 'dPDF-Windows-desktop' } else { 'dPDF-Windows' }
if (!$Desktop -and (Get-Service -Name "actions.runner.*.$expectedName" -ErrorAction SilentlyContinue)) {
    throw 'This dPDF service already exists; it will not be replaced.'
}
$git = (Get-Command git.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
if (!$Desktop -and !$git.StartsWith($env:ProgramFiles, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Service mode needs machine-wide Git for Windows, accessible to NETWORK SERVICE.'
}
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (!(Test-Path -LiteralPath $vswhere)) { throw 'Install the Visual Studio C++ build tools and Windows SDK first.' }
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vs) { throw 'The existing Visual Studio installation lacks the x64 C++ workload. No toolchain was modified.' }
New-Item -ItemType Directory -Path $Root -Force | Out-Null
if (!$Desktop) {
    # Language-independent SIDs: SYSTEM, Administrators, NETWORK SERVICE.
    & icacls.exe $Root /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-20:(OI)(CI)F' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Unable to restrict runner directory ACLs.' }
}
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
function Get-Verified([string]$Url, [string]$File, [string]$Sha256) {
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $File
    if ((Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Sha256) {
        throw "Checksum mismatch: $File"
    }
}
$runnerZip = Join-Path $Root 'runner.zip'
Get-Verified 'https://github.com/actions/runner/releases/download/v2.338.2/actions-runner-win-x64-2.338.2.zip' $runnerZip 'a1f17aa987fc8158bc7521d284912267a0d01b7c2c4684b33c86783b5e8221cc'
Expand-Archive -LiteralPath $runnerZip -DestinationPath $Root
Remove-Item -LiteralPath $runnerZip
$tools = Join-Path $Root 'tools'
$pythonDir = Join-Path $tools 'python'
New-Item -ItemType Directory -Path $pythonDir -Force | Out-Null
$pythonZip = Join-Path $tools 'python.zip'
Get-Verified 'https://www.python.org/ftp/python/3.14.10/python-3.14.10-embed-amd64.zip' $pythonZip '90905e64a840557d8c1bc4f5b74db3d7ee85e3ffb217611cac1c7813acea4159'
Expand-Archive -LiteralPath $pythonZip -DestinationPath $pythonDir
Remove-Item -LiteralPath $pythonZip
$cargoHome = Join-Path $tools 'cargo'
$rustupHome = Join-Path $tools 'rustup'
$rustToolchain = 'stable-x86_64-pc-windows-msvc'
$init = Join-Path $tools 'rustup-init.exe'
Get-Verified 'https://static.rust-lang.org/rustup/archive/1.29.2/x86_64-pc-windows-msvc/rustup-init.exe' $init '00f139da614dced87d3c3d4121c43eebdac3c1bf8a5ee86b9d80dfd71c8d70fd'
$oldRustEnvironment = @{}
foreach ($name in @('CARGO_HOME', 'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN')) {
    $oldRustEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
try {
    $env:CARGO_HOME = $cargoHome
    $env:RUSTUP_HOME = $rustupHome
    $env:RUSTUP_TOOLCHAIN = $rustToolchain
    & $init -y --no-modify-path --profile minimal --default-toolchain stable-x86_64-pc-windows-msvc --component clippy,rustfmt
    if ($LASTEXITCODE -ne 0) { throw 'Isolated Rust setup failed.' }
} finally {
    foreach ($name in $oldRustEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $oldRustEnvironment[$name], 'Process')
    }
}
Remove-Item -LiteralPath $init
$path = "$pythonDir;$cargoHome\bin;$(Split-Path -Parent $git);$([Environment]::GetEnvironmentVariable('PATH', 'Machine'))"
$lines = @(
    "PATH=$path", "CARGO_HOME=$cargoHome", "RUSTUP_HOME=$rustupHome",
    "RUSTUP_TOOLCHAIN=$rustToolchain", "DPDF_CI_ROOT=$tools",
    "DPDF_CI_PYTHON=$pythonDir\python.exe", 'CARGO_BUILD_JOBS=1', 'RUST_TEST_THREADS=1'
)
# Runner.Listener loads this private file. No global environment/registry changes.
[IO.File]::WriteAllLines((Join-Path $Root '.env'), $lines, (New-Object Text.UTF8Encoding($false)))
$runnerName = if ($Desktop) { 'dPDF-Windows-desktop' } else { 'dPDF-Windows' }
$label = if ($Desktop) { 'dpdf-windows-desktop' } else { 'dpdf-windows' }
$token = $null
$secureToken = $null
$pointer = [IntPtr]::Zero
$gh = Get-Command gh.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
if ($gh) {
    # Use an existing local GitHub login; never extract or persist its stored PAT.
    try {
        $token = & $gh.Source api --method POST repos/w3fans/dPDF/actions/runners/registration-token --jq .token 2>$null
        if ($LASTEXITCODE -ne 0) { $token = $null }
    } catch { $token = $null }
}
if (!$token) {
    Write-Host 'Get a short-lived registration token from:'
    Write-Host 'https://github.com/w3fans/dPDF/settings/actions/runners/new'
    Write-Host 'Paste only the registration token at the hidden prompt; never a personal access token.'
    $secureToken = Read-Host 'Runner registration token (valid about one hour)' -AsSecureString
    $pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureToken)
    $token = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
}
$oldInputToken = [Environment]::GetEnvironmentVariable('ACTIONS_RUNNER_INPUT_TOKEN', 'Process')
Push-Location $Root
try {
    # Environment input keeps the token out of process command-line arguments.
    $env:ACTIONS_RUNNER_INPUT_TOKEN = $token
    $arguments = @('--unattended', '--url', 'https://github.com/w3fans/dPDF', '--name', $runnerName,
                   '--labels', $label, '--work', '_work')
    if (!$Desktop) { $arguments += @('--runasservice', '--windowslogonaccount', 'NT AUTHORITY\NETWORK SERVICE') }
    & .\config.cmd @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Runner registration failed. Existing runners were not replaced.' }
} finally {
    [Environment]::SetEnvironmentVariable('ACTIONS_RUNNER_INPUT_TOKEN', $oldInputToken, 'Process')
    $oldInputToken = $null
    $token = $null
    if ($pointer -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer) }
    if ($secureToken) { $secureToken.Dispose() }
    Pop-Location
}
if ($Desktop) {
    Write-Host "Ready. In the dedicated logged-in test account, run $Root\run.cmd when desktop testing is wanted."
    Write-Host 'No automatic login, scheduled task, or interactive service has been created.'
} else {
    Write-Host 'Repository-specific dPDF-Windows service registered. Every new release tag queues validation automatically.'
}
