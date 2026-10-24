param([Parameter(Mandatory)][string]$Executable)
$ErrorActionPreference = 'Stop'
$Executable = [IO.Path]::GetFullPath($Executable)
$bytes = [IO.File]::ReadAllBytes($Executable)
$pe = [BitConverter]::ToInt32($bytes,0x3c)
if ([BitConverter]::ToUInt32($bytes,$pe) -ne 0x4550) { throw 'Invalid PE signature' }
if ([BitConverter]::ToUInt16($bytes,$pe+4) -ne 0x8664) { throw 'Expected Windows x64 executable' }
$optional = $pe+24
if ([BitConverter]::ToUInt16($bytes,$optional) -ne 0x20b) { throw 'Expected PE32+' }
if ([BitConverter]::ToUInt16($bytes,$optional+68) -ne 2) { throw 'Expected Windows GUI subsystem' }
if ([BitConverter]::ToUInt32($bytes,$optional+112+14*8) -ne 0) { throw 'Unexpected CLR runtime header' }
$count = [BitConverter]::ToUInt16($bytes,$pe+6)
$sections = $optional+[BitConverter]::ToUInt16($bytes,$pe+20)
function Convert-Rva([uint32]$rva) {
    for ($i=0; $i -lt $count; $i++) {
        $section = $sections+40*$i
        $address = [BitConverter]::ToUInt32($bytes,$section+12)
        $size = [Math]::Max([BitConverter]::ToUInt32($bytes,$section+8),[BitConverter]::ToUInt32($bytes,$section+16))
        if ($rva -ge $address -and $rva -lt $address+$size) { return [int]($rva-$address+[BitConverter]::ToUInt32($bytes,$section+20)) }
    }
    throw "RVA outside PE sections: $rva"
}
$descriptor = Convert-Rva ([BitConverter]::ToUInt32($bytes,$optional+120))
$imports = @()
while ($true) {
    $nameRva = [BitConverter]::ToUInt32($bytes,$descriptor+12)
    if (!$nameRva) { break }
    $start = Convert-Rva $nameRva
    $end = $start
    while ($bytes[$end] -ne 0) { $end++ }
    $imports += [Text.Encoding]::ASCII.GetString($bytes,$start,$end-$start)
    $descriptor += 20
}
if ($imports | Where-Object { $_ -match '^(vcruntime|msvcp|mscoree|hostfxr|swift).*\.dll$' }) { throw 'Unexpected external application runtime dependency' }
$version = [Diagnostics.FileVersionInfo]::GetVersionInfo($Executable)
if ($version.ProductName -ne 'Cat Codex Switcher') { throw 'Missing Windows product metadata' }
"PASS: Windows x64 GUI, no CLR header, static CRT, product version $($version.FileVersion)"
"Imports: $($imports -join ', ')"
