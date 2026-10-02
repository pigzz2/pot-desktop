param(
    [ValidateSet('Env', 'Check', 'Test', 'Dev', 'Build')]
    [string]$Task = 'Check',
    [ValidateRange(1, 128)]
    [int]$Jobs = 4
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent

# Also works in shells opened before the tools were installed.
foreach ($variable in @('CARGO_HOME', 'RUSTUP_HOME')) {
    $userValue = [Environment]::GetEnvironmentVariable($variable, 'User')
    if ($userValue) {
        [Environment]::SetEnvironmentVariable($variable, $userValue, 'Process')
    }
}
if ($env:CARGO_HOME) {
    $env:PATH = (Join-Path $env:CARGO_HOME 'bin') + ';' + $env:PATH
}
$env:PATH = [Environment]::GetEnvironmentVariable('Path', 'User') + ';' + $env:PATH
$env:CARGO_TARGET_DIR = Join-Path $projectRoot 'src-tauri/target'
$env:CARGO_BUILD_JOBS = $Jobs.ToString()

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (!(Test-Path -LiteralPath $vswhere)) {
    throw 'Install Visual Studio C++ Build Tools and a Windows SDK first.'
}
$vsPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vsPath) {
    throw 'No complete Visual Studio installation with the x64 C++ compiler was found.'
}
$devShell = Join-Path $vsPath 'Common7/Tools/Launch-VsDevShell.ps1'
& $devShell -Arch amd64 -HostArch amd64 -SkipAutomaticLocation

if (!$env:WindowsSdkDir -or !$env:WindowsSDKVersion) {
    throw 'The developer shell could not locate a Windows SDK.'
}
$sdkVersion = $env:WindowsSDKVersion.TrimEnd('\')
$ucrtRoot = if ($env:UniversalCRTSdkDir) { $env:UniversalCRTSdkDir } else { $env:WindowsSdkDir }
$ucrtVersion = if ($env:UCRTVersion) { $env:UCRTVersion.TrimEnd('\') } else { $sdkVersion }
foreach ($sdkFile in @(
    (Join-Path $env:WindowsSdkDir "Lib/$sdkVersion/um/x64/kernel32.lib"),
    (Join-Path $ucrtRoot "Lib/$ucrtVersion/ucrt/x64/ucrt.lib")
)) {
    if (!(Test-Path -LiteralPath $sdkFile)) {
        throw "The Windows SDK installation is incomplete: $sdkFile"
    }
}

foreach ($command in @('cargo', 'rustc', 'pnpm', 'cl.exe', 'link.exe', 'rc.exe')) {
    if (!(Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "Missing build command: $command"
    }
}

if ($Task -eq 'Env') {
    Write-Host 'Windows x64 build environment is ready.'
    return
}

Push-Location $projectRoot
try {
    switch ($Task) {
        'Check' { cargo check --locked --manifest-path src-tauri/Cargo.toml }
        'Test' { cargo test --locked --manifest-path src-tauri/Cargo.toml }
        'Dev' { pnpm tauri dev }
        'Build' { pnpm tauri build }
    }
    if ($LASTEXITCODE -ne 0) {
        throw "$Task failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}
