# Builds and runs the game in a dev build.
#
# Game data comes from the `WoW` link at the repo root (a junction to your own 1.12.1 client,
# gitignored), or from $env:WOW_DATA if you set it. Logs go to logs\run-game.log.
#
#   New-Item -ItemType Junction -Path WoW -Target <your WoW 1.12 folder>   # once
#   .\scripts\run-game.ps1

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $root
if (-not $env:WOW_DATA -and -not (Test-Path (Join-Path $root 'WoW\Data'))) {
    throw "No game data: create the WoW junction at the repo root or set WOW_DATA."
}
New-Item -ItemType Directory -Force (Join-Path $root 'logs') | Out-Null
cargo run --profile play -p esurv 2>&1 | Tee-Object -FilePath (Join-Path $root 'logs\run-game.log')
