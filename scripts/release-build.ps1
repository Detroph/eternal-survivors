# The release build recipe (design.md step 9, item 7): the player build of the game and its
# setup, made through a neutral junction so no path of this machine is compiled in, then the leak
# scan of both executables. Self-contained: it sets its own rustflags and ignores the machine's.
#
#   .\scripts\release-build.ps1            # build and scan; exit code 0 only when both are clean
#
# * `C:\esurv-build` is a junction to this repository: compile-time `CARGO_MANIFEST_DIR` and
#   `OUT_DIR` strings cannot be remapped, so the build runs from a path that names nobody.
# * `--no-default-features` on `esurv` drops `dev` (benilla's build stamp of the project folder,
#   the content watcher, the console window); `esurv-setup` has no features.
# * The target folder is `target\player` (through the junction), apart from the dev builds, whose
#   rustflags differ.
# Output: `target\player\release\esurv.exe`, `esurv-setup.exe`, the `pack_setup` example and
# `THIRD-PARTY-LICENSES.txt`, the licenses of the crates compiled in (third-party-licenses.ps1).

param([string]$Junction = 'C:\esurv-build')

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).ProviderPath

# the junction: made once, and it must point here
if (Test-Path -LiteralPath $Junction) {
    $item = Get-Item -LiteralPath $Junction -Force
    $target = @($item.Target)[0]
    if ($item.LinkType -ne 'Junction' -or ((Resolve-Path -LiteralPath $target).ProviderPath.TrimEnd('\') -ne $repo.TrimEnd('\'))) {
        throw "$Junction exists and is not a junction to $repo (it is $($item.LinkType) -> $target)"
    }
} else {
    New-Item -ItemType Junction -Path $Junction -Target $repo | Out-Null
    Write-Host "release-build: made the junction $Junction -> $repo"
}

# rustflags, most general rule first: rustc applies the last rule that matches
$sysroot = (& rustc --print sysroot).Trim()
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$flags = @(
    "--remap-path-prefix=$($env:USERPROFILE)=/home",
    "--remap-path-prefix=$(Join-Path $cargoHome 'registry\src')=/cargo/registry",
    "--remap-path-prefix=$(Join-Path $cargoHome 'git\checkouts')=/cargo/git",
    "--remap-path-prefix=$sysroot=/rustup",
    "--remap-path-prefix=$repo=/esurv",
    "--remap-path-prefix=$Junction=/esurv",
    '-Clink-arg=/PDBALTPATH:%_PDB%'
)
# the encoded form keeps a path with spaces whole, and wins over the machine config's rustflags
$env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]0x1f
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '8' }
$targetDir = Join-Path $Junction 'target\player'

# the save key (save/lock.rs): kept off the repository, in the owner's profile; a player build
# without it would seal saves with the public dev key
$keyFile = Join-Path $env:USERPROFILE '.esurv\save-key.txt'
if (-not (Test-Path -LiteralPath $keyFile)) { throw "release-build: no save key at $keyFile" }
$env:ESURV_SAVE_KEY = (Get-Content -LiteralPath $keyFile -Raw).Trim()

Push-Location $Junction
try {
    & cargo build --release --target-dir $targetDir -p esurv -p esurv-setup --no-default-features
    if ($LASTEXITCODE -ne 0) { throw 'release-build: the player build failed' }
    & cargo build --release --target-dir $targetDir -p esurv-setup --example pack_setup
    if ($LASTEXITCODE -ne 0) { throw 'release-build: the setup packer did not build' }
} finally {
    Pop-Location
    Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue
    Remove-Item Env:ESURV_SAVE_KEY -ErrorAction SilentlyContinue
}

$out = Join-Path $repo 'target\player\release'
# the licenses of the crates compiled in, shipped beside the game (scripts\third-party-licenses.ps1)
$licenses = Join-Path $out 'THIRD-PARTY-LICENSES.txt'
& (Join-Path $PSScriptRoot 'third-party-licenses.ps1') -Out $licenses
if ($LASTEXITCODE -ne 0) {
    Write-Host 'release-build: FAIL, the third-party licenses were not written'
    exit 1
}
& (Join-Path $PSScriptRoot 'leak-scan.ps1') (Join-Path $out 'esurv.exe') (Join-Path $out 'esurv-setup.exe') $licenses
if ($LASTEXITCODE -ne 0) {
    Write-Host 'release-build: FAIL, the leak scan found something in the executables or the licenses'
    exit 1
}
Write-Host "release-build: PASS, $out"
exit 0
