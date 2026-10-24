param([string]$Executable = '', [string]$Output = '')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (!$Executable) { $Executable = Join-Path $projectRoot 'target\release\Cat-Codex-Switcher.exe' }
if (!$Output) { $Output = Join-Path $projectRoot 'artifacts\ui' }
$Executable = [IO.Path]::GetFullPath($Executable)
$Output = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path $Output | Out-Null
$cases = @()
foreach ($language in @('uk', 'en', 'ru')) {
    foreach ($theme in @('light', 'dark')) {
        foreach ($scale in @('1', '1.5', '2')) {
            $cases += @{ language=$language; theme=$theme; scale=$scale; page='accounts'; empty=$false }
        }
    }
    foreach ($page in @('settings', 'manage', 'switch', 'remove')) {
        $cases += @{ language=$language; theme='dark'; scale='1.5'; page=$page; empty=$false }
    }
}
foreach ($theme in @('light','dark')) { $cases += @{ language='uk'; theme=$theme; scale='1'; page='accounts'; empty=$true } }
foreach ($case in $cases) {
    $suffix = if ($case.empty) { '-empty' } else { '' }
    $name = "$($case.language)-$($case.theme)-$($case.scale)-$($case.page)$suffix.png"
    $destination = Join-Path $Output $name
    $arguments = @('--ui-fixture','--language',$case.language,'--theme',$case.theme,'--scale',$case.scale,'--page',$case.page,'--screenshot',('"' + $destination + '"'),'--quit-after-capture')
    if ($case.empty) { $arguments += '--empty' }
    $process = Start-Process -FilePath $Executable -ArgumentList $arguments -WindowStyle Hidden -PassThru
    if (!$process.WaitForExit(20000)) { Stop-Process -Id $process.Id; throw "Screenshot fixture timed out: $name" }
    if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath $destination)) { throw "Screenshot fixture failed: $name" }
}
"Captured $($cases.Count) fictional UI cases in $Output"
