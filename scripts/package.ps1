param([switch]$SkipChecks)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (!$SkipChecks) {
        cargo fmt --check
        if ($LASTEXITCODE) { throw 'Formatting check failed' }
        cargo clippy --locked --all-targets -- -D warnings
        if ($LASTEXITCODE) { throw 'Clippy failed' }
        cargo test --locked --all-targets
        if ($LASTEXITCODE) { throw 'Tests failed' }
    }
    cargo build --locked --release --bin Cat-Codex-Switcher
    if ($LASTEXITCODE) { throw 'Release build failed' }
    $destination = Join-Path $projectRoot 'dist'
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\Cat-Codex-Switcher.exe') -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $destination -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'PARITY.md') -Destination $destination -Force
    & (Join-Path $PSScriptRoot 'third-party-notices.ps1') -Output (Join-Path $destination 'THIRD-PARTY-NOTICES.txt')
    $previousResult = $env:CAT_SWITCHER_SELF_TEST_RESULT
    try {
        $env:CAT_SWITCHER_SELF_TEST_RESULT = Join-Path $destination 'self-test.txt'
        $process = Start-Process -FilePath (Join-Path $destination 'Cat-Codex-Switcher.exe') -ArgumentList '--self-test' -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw 'Packaged self-test failed' }
    } finally { $env:CAT_SWITCHER_SELF_TEST_RESULT = $previousResult }
    $hash = (Get-FileHash -LiteralPath (Join-Path $destination 'Cat-Codex-Switcher.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $destination 'Cat-Codex-Switcher.exe.sha256'), "$hash  Cat-Codex-Switcher.exe`n")
    "Built portable EXE: $destination"
} finally { Pop-Location }
