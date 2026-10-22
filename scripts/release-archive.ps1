# The release archive (design.md step 9, item 11): `dist\eternal-survivors-<version>.zip`, holding
# `Eternal Survivors Setup.exe` (the game, its content, README and licenses carried inside it),
# the README and the licenses (ours, and the crates' in THIRD-PARTY-LICENSES.txt). The leak scan of the archive is its gate: an archive that fails it
# is deleted, never handed out.
#
#   .\scripts\release-archive.ps1 [-Version v0.2.0] [-SkipBuild] [-Junction C:\esurv-build]
#
# -Version defaults to the workspace version (Cargo.toml's `[workspace.package] version`, the
# version the game's build line shows), with a `v`. -SkipBuild packs the last release build
# (`scripts\release-build.ps1`) instead of building again; -Junction goes to that build.

param(
    [string]$Version = '',
    [switch]$SkipBuild,
    [string]$Junction = 'C:\esurv-build'
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).ProviderPath
$bin = Join-Path $repo 'target\player\release'
if (-not $Version) {
    # the workspace version: the first `version = "x.y.z"` of Cargo.toml, `[workspace.package]`'s
    $line = Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version = "(\d+\.\d+\.\d+)"' | Select-Object -First 1
    if (-not $line) { throw 'release-archive: no workspace version in Cargo.toml' }
    $Version = 'v' + $line.Matches[0].Groups[1].Value
}

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'release-build.ps1') -Junction $Junction
    if ($LASTEXITCODE -ne 0) { Write-Host 'release-archive: FAIL, the release build did not pass'; exit 1 }
}
foreach ($exe in 'esurv.exe', 'esurv-setup.exe', 'examples\pack_setup.exe', 'THIRD-PARTY-LICENSES.txt') {
    if (-not (Test-Path (Join-Path $bin $exe))) { throw "release-archive: no $exe in $bin; run scripts\release-build.ps1" }
}

# the game folder the setup writes, staged
$stage = Join-Path $repo 'target\player\stage'
$game = Join-Path $stage 'game'
$box = Join-Path $stage 'archive'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $game, $box | Out-Null
Copy-Item (Join-Path $bin 'esurv.exe') (Join-Path $game 'Eternal Survivors.exe')
# the content without its comments (ours, documentation; they may name what the client holds),
# each file checked to read as its original does and the staged folder to load (stage_content)
Push-Location $repo
try {
    $prev = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    & cargo run --release -q -p esurv-content --example stage_content -- (Join-Path $repo 'content') (Join-Path $game 'content')
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prev
} finally { Pop-Location }
if ($code -ne 0) { Write-Host 'release-archive: FAIL, the content did not stage'; exit 1 }
$shipped = @{
    'README.txt'         = Join-Path $repo 'release\README.txt'
    'NOTICE.txt'         = Join-Path $repo 'release\NOTICE.txt'
    'LICENSE-MIT.txt'    = Join-Path $repo 'LICENSE-MIT'
    'LICENSE-APACHE.txt' = Join-Path $repo 'LICENSE-APACHE'
    # the crates' licenses, written by the release build (scripts\third-party-licenses.ps1)
    'THIRD-PARTY-LICENSES.txt' = Join-Path $bin 'THIRD-PARTY-LICENSES.txt'
}
foreach ($name in $shipped.Keys) {
    Copy-Item $shipped[$name] (Join-Path $game $name)
    Copy-Item $shipped[$name] (Join-Path $box $name)
}
Set-Content -Path (Join-Path $game 'VERSION.txt') -Value $Version -Encoding ascii -NoNewline

# the one-file setup: the setup executable with the staged game folder appended
$setup = Join-Path $box 'Eternal Survivors Setup.exe'
& (Join-Path $bin 'examples\pack_setup.exe') (Join-Path $bin 'esurv-setup.exe') $game $setup
if ($LASTEXITCODE -ne 0) { throw 'release-archive: the setup was not packed' }

$dist = Join-Path $repo 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$zip = Join-Path $dist "eternal-survivors-$Version.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($box, $zip, [IO.Compression.CompressionLevel]::Optimal, $false)

& (Join-Path $PSScriptRoot 'leak-scan.ps1') $zip
if ($LASTEXITCODE -ne 0) {
    Remove-Item -Force $zip
    Write-Host "release-archive: FAIL, the leak scan found something; the archive was deleted"
    exit 1
}
$size = (Get-Item $zip).Length
Write-Host ("release-archive: PASS, {0} ({1:N1} MB)" -f $zip, ($size / 1MB))
exit 0
