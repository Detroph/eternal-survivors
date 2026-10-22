# The third-party licenses (design.md step 9): THIRD-PARTY-LICENSES.txt, the licenses of every Rust
# crate compiled into the player build (the game and its setup), shipped beside the game and in
# the release archive. Run by scripts\release-build.ps1; offline, from the cargo registry's own
# copies of the crates.
#
#   .\scripts\third-party-licenses.ps1 [-Out <file>]
#
# * The crates: `cargo tree -p esurv -p esurv-setup --no-default-features -e normal` (what the
#   release build compiles in), our own workspace crates left out (their license is ours,
#   LICENSE-MIT and LICENSE-APACHE, shipped already), plus Lua 5.1.5, which lua-src (a build
#   dependency) compiles into the game.
# * The texts: each crate's own license files (LICENSE*, LICENCE*, COPYING*, NOTICE*, COPYRIGHT*,
#   UNLICENSE*) from its package. A text that several crates share word for word (whitespace
#   aside) is written once, with every crate's copyright lines above it. A crate whose package
#   carries no license file gets the standard text of the MIT or Apache-2.0 license it declares,
#   with its Cargo.toml authors (names only) as its notice.
# The exit code: 0 written, 1 a crate with no license text to give.

param([string]$Out = '')

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).ProviderPath
if (-not $Out) { $Out = Join-Path $repo 'target\player\release\THIRD-PARTY-LICENSES.txt' }
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$registry = Get-ChildItem -Directory (Join-Path $cargoHome 'registry\src') | Where-Object { $_.Name -like 'index.crates.io-*' } | Select-Object -First 1
if (-not $registry) { throw 'third-party-licenses: no crates.io registry sources under CARGO_HOME' }

# the crates, as `name vX.Y.Z|license` (a path dependency with its folder in brackets)
Push-Location $repo
try {
    $prev = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    $lines = & cargo tree -p esurv -p esurv-setup --no-default-features -e normal --prefix none --format '{p}|{l}' 2>$null
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prev
    if ($code -ne 0) { throw 'third-party-licenses: cargo tree failed' }
} finally { Pop-Location }

$crates = @{}
foreach ($line in $lines) {
    $line = ($line -replace ' \(\*\)', '' -replace ' \(proc-macro\)', '').Trim()
    if (-not $line) { continue }
    $p, $license = $line.Split('|', 2)
    if ($p -notmatch '^(\S+) v(\S+)(?: \((.+)\))?$') { continue }
    $name = $Matches[1]; $version = $Matches[2]; $path = $Matches[3]
    if ($path) {
        $full = (Resolve-Path -LiteralPath $path).ProviderPath
        if ($full.StartsWith((Join-Path $repo 'crates'), [StringComparison]::OrdinalIgnoreCase)) { continue }
        $dir = $full
    } else {
        $dir = Join-Path $registry.FullName "$name-$version"
    }
    $crates["$name $version"] = [pscustomobject]@{ Name = $name; Version = $version; License = $license.Trim(); Dir = $dir }
}

# the standard texts, for a package with no license file: ours, without our copyright line
$standard = @{
    'MIT'        = ([IO.File]::ReadAllText((Join-Path $repo 'LICENSE-MIT')) -replace '(?m)^Copyright \(c\).*\r?\n', '')
    'Apache-2.0' = [IO.File]::ReadAllText((Join-Path $repo 'LICENSE-APACHE'))
}

# a copyright line: "Copyright", "(c) <year>" or the copyright sign (U+00A9) at its start, with a
# year, "(c)" or the sign in it (not Apache's appendix template, not its clause "(c) You must
# retain ...", not a wrapped "copyright notice, ..." of a clause)
function Test-Copyright([string]$l) {
    $t = $l.Trim()
    $sign = [string][char]0xA9
    ($t -match ('^(?i)(copyright\b|\(c\)\s*\d|' + $sign + ')')) -and ($t -match ('(?i)(\b(19|20)\d\d\b|\(c\)|' + $sign + ')')) -and ($t -notmatch '\[yyyy\]')
}

function Get-Kind([string]$body, [string]$file) {
    $body = $body -replace '\s+', ' '
    if ($body -cmatch '^\s*GNU GENERAL PUBLIC LICENSE') { return 'GNU GPL (an alternative the crate also offers; used under its permissive license)' }
    if ($body -match 'Apache License' -and $body -match 'Version 2\.0') { return 'Apache License 2.0' }
    if ($body -match 'Boost Software License') { return 'Boost Software License 1.0' }
    if ($body -match 'Permission is hereby granted, free of charge') {
        if ($body -match 'shall be included in all') { return 'MIT License' } else { return 'MIT No Attribution' }
    }
    if ($body -match 'Mozilla Public License') { return 'Mozilla Public License 2.0' }
    if ($body -match 'This is free and unencumbered software released into the public domain') { return 'The Unlicense' }
    if ($body -match 'Creative Commons' -and $body -match 'CC0') { return 'CC0 1.0' }
    if ($body -match 'UNICODE LICENSE|Unicode, Inc\.|UNICODE, INC\.') { return 'Unicode License' }
    if ($body -match 'Permission to use, copy, modify, and/or distribute this software for any') { return 'ISC / 0BSD License' }
    if ($body -match 'This software is provided .as-is., without any express or implied') { return 'zlib License' }
    if ($body -match 'Redistribution and use in source and binary forms') { return 'BSD License' }
    if ($file -match '(?i)^notice') { return 'Notice' }
    return "the crate's own $file"
}

$groups = [ordered]@{}   # normalised body -> @{ Kind; Text; Users = list of "crate: notice" }
$missing = @()

function Add-Text($crateLabel, [string]$text, [string]$file, [string[]]$extraNotice) {
    $text = $text -replace "`r`n", "`n" -replace "`r", "`n"
    $all = $text.Split("`n")
    $notices = @($all | Where-Object { Test-Copyright $_ } | ForEach-Object { $_.Trim() })
    $body = (($all | Where-Object { -not (Test-Copyright $_) }) -join "`n").Trim("`n", ' ', "`t")
    $key = ($body -replace '\s+', ' ').Trim()
    if (-not $groups.Contains($key)) {
        $groups[$key] = [pscustomobject]@{ Kind = (Get-Kind $body $file); Text = $body; Users = New-Object System.Collections.Generic.List[string] }
    }
    if ($notices.Count -eq 0) { $notices = $extraNotice }
    $groups[$key].Users.Add(("{0}: {1}" -f $crateLabel, ($notices -join ' / ')))
}

function Get-Authors($dir) {
    $toml = Join-Path $dir 'Cargo.toml'
    if (-not (Test-Path $toml)) { return '' }
    $t = [IO.File]::ReadAllText($toml)
    if ($t -notmatch '(?s)(?:^|\n)authors\s*=\s*\[(.*?)\]') { return '' }
    # names only: an address in angle brackets or bare is left out (the license texts carry theirs)
    $names = [regex]::Matches($Matches[1], '"([^"]*)"') | ForEach-Object { ($_.Groups[1].Value -replace '\s*<[^>]*>', '' -replace '\s*\S+@\S+', '').Trim() } | Where-Object { $_ }
    return ($names -join ', ')
}

$licenseFile = '^(?i)(licen[cs]e|copying|notice|copyright|unlicense)([-._].*)?$'
# the standard text of a license some package carries no file of, from another package that does
$spdxKinds = [ordered]@{ 'MIT' = 'MIT License'; 'Apache-2.0' = 'Apache License 2.0'; 'BSL-1.0' = 'Boost Software License 1.0'; 'CC0-1.0' = 'CC0 1.0'; 'Zlib' = 'zlib License'; 'MIT-0' = 'MIT No Attribution' }
foreach ($c in $crates.Values) {
    foreach ($f in @(Get-ChildItem -LiteralPath $c.Dir -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match $licenseFile })) {
        $t = [IO.File]::ReadAllText($f.FullName) -replace "`r`n", "`n"
        $body = (($t.Split("`n") | Where-Object { -not (Test-Copyright $_) }) -join "`n").Trim()
        foreach ($spdx in $spdxKinds.Keys) {
            if (-not $standard.ContainsKey($spdx) -and (Get-Kind $body $f.Name) -eq $spdxKinds[$spdx]) { $standard[$spdx] = $body }
        }
    }
}
foreach ($c in ($crates.Values | Sort-Object Name, Version)) {
    $label = "$($c.Name) $($c.Version)"
    $authors = Get-Authors $c.Dir
    $noAuthors = if ($authors) { "(no copyright line in its license text; authors: $authors)" } else { '(no copyright line in its license text)' }
    $files = @(Get-ChildItem -LiteralPath $c.Dir -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match $licenseFile } | Sort-Object Name)
    if ($files.Count -gt 0) {
        foreach ($f in $files) { Add-Text $label ([IO.File]::ReadAllText($f.FullName)) $f.Name @($noAuthors) }
    } else {
        $kinds = @($spdxKinds.Keys | Where-Object { $standard.ContainsKey($_) -and $c.License -match ('(^|[^\w.-])' + [regex]::Escape($_) + '($|[^\w.-])') })
        if ($kinds.Count -eq 0) { $missing += "$label ($($c.License))"; continue }
        $notice = if ($authors) { "Copyright the $($c.Name) authors: $authors (its package carries no license file; the standard text of its license)" } else { "Copyright the $($c.Name) authors (its package carries no license file; the standard text of its license)" }
        foreach ($k in $kinds) { Add-Text $label $standard[$k] $k @($notice) }
    }
}

# Lua 5.1.5, compiled into the game by lua-src (a build dependency): its license is in lua.h
$luaH = Join-Path $repo 'third_party\lua-src\lua-5.1.5\lua.h'
$luaCount = 0
if (Test-Path $luaH) {
    $h = [IO.File]::ReadAllText($luaH)
    if ($h -match '(?s)/\*{10,}\s*\n(\* Copyright.*?)\*{10,}/') {
        $text = ($Matches[1] -split "`n" | ForEach-Object { $_ -replace '^\* ?', '' }) -join "`n"
        Add-Text 'Lua 5.1.5 (built into the game by lua-src)' $text 'lua.h' @('')
        $luaCount = 1
    }
}
if ($luaCount -eq 0) { $missing += 'Lua 5.1.5 (no license block found in third_party\lua-src\lua-5.1.5\lua.h)' }

if ($missing.Count -gt 0) {
    Write-Host "third-party-licenses: FAIL, no license text for:"
    $missing | ForEach-Object { Write-Host "  $_" }
    exit 1
}

# the file
$sb = New-Object Text.StringBuilder
$nl = "`r`n"
function W([string]$s = '') { [void]$sb.Append(($s -replace "`n", $nl)).Append($nl) }
$total = $crates.Count + $luaCount
W 'Eternal Survivors: third-party licenses'
W '========================================'
W ''
W "Eternal Survivors (the game and its setup) is built with the $($crates.Count) open-source Rust crates"
W 'listed below, and with Lua 5.1.5, which is built into the game. Each is listed with the license'
W 'it declares; the license texts follow, each written once, with the copyright notices of every'
W 'crate that uses it. Where a crate offers a choice of licenses, every license text its package'
W 'carries is given here.'
W ''
W "Crates ($total)"
W ('-' * "Crates ($total)".Length)
foreach ($c in ($crates.Values | Sort-Object Name, Version)) { W ("  {0} {1}  ({2})" -f $c.Name, $c.Version, $(if ($c.License) { $c.License } else { 'see its license text' })) }
if ($luaCount) { W '  Lua 5.1.5  (MIT)' }
$i = 0
foreach ($g in $groups.Values) {
    $i++
    W ''
    W ''
    W ('=' * 78)
    W ("License text {0} of {1}: {2}, for {3} crate(s)" -f $i, $groups.Count, $g.Kind, $g.Users.Count)
    W ('=' * 78)
    W ''
    foreach ($u in $g.Users) { W "  $u" }
    W ''
    W $g.Text
}
New-Item -ItemType Directory -Force (Split-Path -Parent $Out) | Out-Null
[IO.File]::WriteAllText($Out, $sb.ToString(), (New-Object Text.UTF8Encoding($false)))
$kb = (Get-Item $Out).Length / 1KB
Write-Host ("third-party-licenses: {0} ({1} crates and Lua, {2} license texts, {3:N0} KB)" -f $Out, $crates.Count, $groups.Count, $kb)
exit 0
