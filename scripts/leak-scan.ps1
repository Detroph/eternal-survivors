# The leak scan: the pass/fail check every release item goes through (design.md step 9, item 6).
#
#   .\scripts\leak-scan.ps1 <file | folder | .zip> [...]
#
# It reads every file (a folder's, a zip's entries, and the files a setup executable carries in
# its payload) and reports:
#   * a user name of this machine (`$env:USERNAME`, and -UserNames) anywhere in the bytes;
#   * an absolute path into a user folder (C:\Users\<name>\, /home/<name>/, /Users/<name>/);
#   * an e-mail address not on the allow-list (noreply addresses, and -AllowEmails);
#   * a Blizzard file: by type (MPQ, BLP, M2, WMO, ADT, WDT, DBC, WAV, MP3, OGG, TTF, and .xml or
#     .lua from a client's Interface, FrameXML or GlueXML) or by its first bytes (MPQ, BLP, M2,
#     WMO/ADT, DBC, RIFF WAVE, MP3, Ogg), whatever its name.
# Strings are searched as single bytes and as UTF-16 at both alignments. The exit code is the
# gate: 0 clean, 1 something found, 2 a path that does not exist.

param(
    [Parameter(Mandatory = $true, Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$Path,
    [string[]]$UserNames = @(),
    [string[]]$AllowEmails = @(),
    [int]$Show = 12
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$names = @($UserNames + $env:USERNAME) | Where-Object { $_ } | Sort-Object -Unique
$userRegex = ($names | ForEach-Object { [regex]::Escape($_) }) -join '|'
$checks = @(
    @{ Kind = 'user name'; Regex = [regex]::new("(?i)($userRegex)") },
    @{ Kind = 'user folder path'; Regex = [regex]::new('(?i)\b[a-z]:[\\/]{1,2}(users|documents and settings)[\\/]{1,2}(?!public[\\/]|default[\\/]|all users[\\/])[a-z0-9 ._-]{2,}[\\/]') },
    @{ Kind = 'user folder path'; Regex = [regex]::new('(?<![a-z0-9_.])/(home|Users)/(?!runner/)[A-Za-z0-9._-]{2,}/') },
    @{ Kind = 'e-mail address'; Regex = [regex]::new('(?i)(?<![a-z0-9._%+-])[a-z0-9][a-z0-9._%+-]{1,63}@[a-z0-9-]{2,63}(\.[a-z0-9-]{1,63})*\.[a-z]{2,10}(?![a-z0-9])') }
)
# Third-party copyright lines compiled in, which must ship with the code they credit: bevy's SMAA
# shader's license header (bevy_anti_alias, smaa.wgsl).
$thirdPartyCredits = @('joseignacioechevarria@gmail.com', 'fernandn@microsoft.com', 'jorge@iryoku.com', 'diegog@unizar.es', 'bmasia@unizar.es')
$allowEmail = @('noreply@', 'no-reply@') + $thirdPartyCredits + $AllowEmails
$blizzardTypes = @('.mpq', '.blp', '.m2', '.mdx', '.wmo', '.adt', '.wdt', '.wdl', '.dbc', '.wav', '.mp3', '.ogg', '.ttf', '.otf', '.skin', '.anim')
$latin1 = [Text.Encoding]::GetEncoding(28591)
$utf16 = [Text.Encoding]::Unicode
$findings = New-Object System.Collections.Generic.List[object]

function Add-Finding($item, $kind, $what) {
    $findings.Add([pscustomobject]@{ Item = $item; Kind = $kind; What = $what })
}

function Test-Magic([byte[]]$b) {
    if ($b.Length -lt 4) { return $null }
    $head = $latin1.GetString($b, 0, [Math]::Min(12, $b.Length))
    if ($head.StartsWith("MPQ$([char]0x1a)")) { return 'MPQ archive' }
    if ($head.StartsWith('BLP1') -or $head.StartsWith('BLP2')) { return 'BLP texture' }
    if ($head.StartsWith('MD20')) { return 'M2 model' }
    if ($head.StartsWith('REVM')) { return 'WMO/ADT/WDT chunk file' }
    if ($head.StartsWith('WDBC')) { return 'DBC table' }
    if ($head.StartsWith('RIFF') -and $head.Length -ge 12 -and $head.Substring(8, 4) -eq 'WAVE') { return 'WAV sound' }
    if ($head.StartsWith('ID3')) { return 'MP3 sound' }
    if ($head.StartsWith('OggS')) { return 'Ogg sound' }
    return $null
}

function Test-Name($item) {
    $lower = $item.ToLowerInvariant()
    $ext = [IO.Path]::GetExtension($lower)
    if ($blizzardTypes -contains $ext) { Add-Finding $item 'Blizzard file type' $ext }
    if (($ext -eq '.xml' -or $ext -eq '.lua' -or $ext -eq '.toc') -and $lower -match '(^|[\\/!])(interface|framexml|gluexml)[\\/]') {
        Add-Finding $item 'client interface file' $ext
    }
}

# Whether the e-mail address at `$at` in `$text` is a crate author's credit in the third-party
# licenses: written `<address>` on a copyright line ("Copyright (c) 2020 Name <address>").
function Test-Credit([string]$text, [int]$at) {
    if ($at -lt 1 -or $text[$at - 1] -ne '<') { return $false }
    $start = $text.LastIndexOf("`n", $at) + 1
    $end = $text.IndexOf("`n", $at)
    if ($end -lt 0) { $end = $text.Length }
    return ($text.Substring($start, $end - $start) -match ('(?i)copyright|\(c\)|' + [string][char]0xA9))
}

function Test-Strings($item, [byte[]]$b) {
    # THIRD-PARTY-LICENSES.txt (scripts\third-party-licenses.ps1) must carry the crates' copyright
    # notices as they are, their authors' addresses too: there, and only there, an address on a
    # copyright line is a credit, not a leak
    $credits = [IO.Path]::GetFileName(($item -split '!')[-1]) -eq 'THIRD-PARTY-LICENSES.txt'
    $texts = @($latin1.GetString($b), $utf16.GetString($b))
    if ($b.Length -gt 1) { $texts += $utf16.GetString($b, 1, $b.Length - 1) }
    $seen = @{}
    foreach ($text in $texts) {
        foreach ($c in $checks) {
            foreach ($m in $c.Regex.Matches($text)) {
                $v = $m.Value
                if ($c.Kind -eq 'e-mail address') {
                    $low = $v.ToLowerInvariant()
                    if ($allowEmail | Where-Object { $low.Contains($_.ToLowerInvariant()) }) { continue }
                    if ($credits -and (Test-Credit $text $m.Index)) { continue }
                }
                # the context around the hit, printable characters only
                $from = [Math]::Max(0, $m.Index - 24)
                $len = [Math]::Min($text.Length - $from, $v.Length + 48)
                $ctx = ($text.Substring($from, $len) -replace '[^\x20-\x7e]', '.')
                $key = "$($c.Kind)|$v"
                if ($seen.ContainsKey($key)) { $seen[$key].Count++ ; continue }
                $seen[$key] = [pscustomobject]@{ Kind = $c.Kind; Value = $v; Context = $ctx; Count = 1 }
            }
        }
    }
    foreach ($s in $seen.Values) {
        Add-Finding $item $s.Kind ("{0} (x{1}) in ...{2}..." -f $s.Value, $s.Count, $s.Context)
    }
}

# The files a setup executable carries after its own bytes (esurv_setup's payload): path, bytes.
function Get-Payload([byte[]]$b) {
    $n = $b.Length
    if ($n -lt 20 -or $latin1.GetString($b, $n - 8, 8) -ne 'ESRVPAY1') { return @() }
    $len = [BitConverter]::ToUInt64($b, $n - 20)
    $count = [BitConverter]::ToUInt32($b, $n - 12)
    $at = [long]($n - 20 - $len)
    $out = @()
    for ($i = 0; $i -lt $count; $i++) {
        $pl = [BitConverter]::ToUInt32($b, $at); $at += 4
        $p = [Text.Encoding]::UTF8.GetString($b, $at, $pl); $at += $pl
        $size = [long][BitConverter]::ToUInt64($b, $at); $at += 8
        $data = New-Object byte[] $size
        [Array]::Copy($b, $at, $data, 0, $size); $at += $size
        $out += , @($p, $data)
    }
    return $out
}

function Test-Item($item, [byte[]]$b) {
    Test-Name $item
    $magic = Test-Magic $b
    if ($magic) { Add-Finding $item 'Blizzard file content' $magic }
    $payload = Get-Payload $b
    if ($payload.Count -gt 0) {
        Write-Host "  $item carries $($payload.Count) files"
        foreach ($p in $payload) { Test-Item "$item!$($p[0])" $p[1] }
        # the stub itself, without its payload
        $stubLen = $b.Length - 20 - [BitConverter]::ToUInt64($b, $b.Length - 20)
        $stub = New-Object byte[] $stubLen
        [Array]::Copy($b, 0, $stub, 0, $stubLen)
        Test-Strings $item $stub
    } else {
        Test-Strings $item $b
    }
}

function Test-Zip($zipPath, $label) {
    $zip = [IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        foreach ($e in $zip.Entries) {
            if ($e.FullName.EndsWith('/')) { continue }
            $item = "$label!$($e.FullName)"
            $ms = New-Object IO.MemoryStream
            $s = $e.Open(); $s.CopyTo($ms); $s.Close()
            Test-Item $item $ms.ToArray()
        }
    } finally { $zip.Dispose() }
}

$missing = $false
foreach ($p in $Path) {
    if (-not (Test-Path -LiteralPath $p)) { Write-Host "leak-scan: no such path: $p"; $missing = $true; continue }
    $full = (Resolve-Path -LiteralPath $p).ProviderPath
    Write-Host "leak-scan: $full"
    if (Test-Path -LiteralPath $full -PathType Container) {
        Test-Name $full
        Get-ChildItem -LiteralPath $full -Recurse -Force -File | ForEach-Object {
            $rel = $_.FullName.Substring($full.Length).TrimStart('\')
            if ($_.Extension -eq '.zip') { Test-Zip $_.FullName $rel }
            else { Test-Item $rel ([IO.File]::ReadAllBytes($_.FullName)) }
        }
    } elseif ($full.ToLowerInvariant().EndsWith('.zip')) {
        Test-Zip $full ([IO.Path]::GetFileName($full))
    } else {
        Test-Item ([IO.Path]::GetFileName($full)) ([IO.File]::ReadAllBytes($full))
    }
}

if ($findings.Count -gt 0) {
    Write-Host "leak-scan: FAIL, $($findings.Count) findings"
    $findings | Group-Object Kind | ForEach-Object {
        Write-Host "  $($_.Name): $($_.Count)"
        $_.Group | Select-Object -First $Show | ForEach-Object { Write-Host "    $($_.Item): $($_.What)" }
    }
    exit 1
}
if ($missing) { exit 2 }
Write-Host 'leak-scan: PASS'
exit 0
