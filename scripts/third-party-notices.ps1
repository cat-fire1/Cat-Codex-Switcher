param([Parameter(Mandatory)][string]$Output)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $metadataText = cargo metadata --offline --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE) { throw 'Could not resolve dependency licenses' }
    $metadata = $metadataText | ConvertFrom-Json
    $notice = [Text.StringBuilder]::new()
    [void]$notice.AppendLine('Cat Codex Switcher — third-party dependency notices')
    [void]$notice.AppendLine('Includes original Codex Account Switcher source/assets under MIT; see LICENSE.')
    foreach ($package in ($metadata.packages | Sort-Object name,version)) {
        if ($package.name -eq 'cat-codex-switcher') { continue }
        [void]$notice.AppendLine("`n$($package.name) $($package.version) — $($package.license)")
        [void]$notice.AppendLine($package.repository)
        $folder = Split-Path -Parent $package.manifest_path
        $licenses = Get-ChildItem -LiteralPath $folder -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE|UNLICENSE|COPYRIGHT)' }
        foreach ($license in $licenses) { [void]$notice.AppendLine($license.Name); [void]$notice.AppendLine([IO.File]::ReadAllText($license.FullName)) }
        if ($package.license_file) {
            $licensePath = Join-Path $folder $package.license_file
            if (Test-Path -LiteralPath $licensePath -PathType Leaf) { [void]$notice.AppendLine([IO.File]::ReadAllText($licensePath)) }
        }
    }
    [IO.File]::WriteAllText([IO.Path]::GetFullPath($Output),$notice.ToString())
} finally { Pop-Location }
