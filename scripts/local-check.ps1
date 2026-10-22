# The local check (the game has no CI yet, so it stands in for one).
# In order: the game crates' tests, the golden run, the client-name scan, then the
# release build and archive with their leak scans. Prints a pass/fail line for each and exits 0
# only when all pass.
#
#   .\scripts\local-check.ps1 [-GoldenHash <hash>] [-Version v0.2.0] [-SkipRelease] [-Junction C:\esurv-build]
#
# -GoldenHash compares the golden run's hash with a recorded one (design.md step 9, item 2);
# without it the hash is printed. -Version defaults to the workspace version (release-archive.ps1);
# -Junction is the release build's neutral path (release-build.ps1). benilla-app's own tests do
# not build on Windows, so only the game's crates are tested. Uses CARGO_BUILD_JOBS (8 when unset).

param(
    [string]$GoldenHash = '',
    [string]$Version = '',
    [switch]$SkipRelease,
    [string]$Junction = 'C:\esurv-build'
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).ProviderPath
Set-Location $repo
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '8' }
$logs = Join-Path $repo 'logs'
New-Item -ItemType Directory -Force $logs | Out-Null
$results = [ordered]@{}

function Step($name, $log, [scriptblock]$body) {
    Write-Host "`n=== $name"
    $start = Get-Date
    $ok = $false
    $note = ''
    try {
        $r = & $body $log
        $ok = $r.Ok; $note = $r.Note
    } catch {
        $note = "$_"
    }
    $secs = [int]((Get-Date) - $start).TotalSeconds
    $results[$name] = [pscustomobject]@{ Ok = $ok; Note = $note; Secs = $secs; Log = $log }
    Write-Host ("=== {0}: {1} {2}" -f $name, $(if ($ok) { 'PASS' } else { 'FAIL' }), $note)
}

# a command's output to the console and a log, its exit code back
function Run($log, [string[]]$cmd) {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $cmd[0] $cmd[1..($cmd.Length - 1)] 2>&1 | ForEach-Object { "$_" } | Tee-Object -Variable lines | Out-Host
    $code = $LASTEXITCODE
    $lines | Set-Content -Path $log -Encoding utf8
    $ErrorActionPreference = $prev
    return $code
}

Step 'tests' (Join-Path $logs 'check-tests.log') {
    param($log)
    $code = Run $log @('cargo', 'test', '--release',
        '-p', 'esurv', '-p', 'esurv-app', '-p', 'esurv-content', '-p', 'esurv-protocol',
        '-p', 'esurv-server', '-p', 'esurv-sim', '-p', 'esurv-client-check', '-p', 'esurv-setup')
    $text = Get-Content $log
    $passed = ($text | Select-String 'test result: ok\. (\d+) passed' -AllMatches | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Measure-Object -Sum).Sum
    $failed = @($text | Select-String 'test result: FAILED').Count
    @{ Ok = ($code -eq 0 -and $failed -eq 0); Note = "($passed passed, exit $code)" }
}

Step 'golden run' (Join-Path $logs 'check-golden-run.log') {
    param($log)
    $code = Run $log @('cargo', 'run', '--release', '-p', 'esurv-sim', '--example', 'golden_run')
    $line = Get-Content $log | Select-String 'golden run \(.*\): ([0-9a-f]{16})' | Select-Object -Last 1
    if (-not $line) { return @{ Ok = $false; Note = "(no hash, exit $code)" } }
    $hash = $line.Matches[0].Groups[1].Value
    if ($GoldenHash) {
        @{ Ok = ($code -eq 0 -and $hash -eq $GoldenHash); Note = "($hash, expected $GoldenHash)" }
    } else {
        @{ Ok = ($code -eq 0); Note = "($hash; pass -GoldenHash to compare)" }
    }
}

Step 'client-name scan' (Join-Path $logs 'check-client-names.log') {
    param($log)
    $code = Run $log @('cargo', 'run', '-p', 'esurv-content', '--features', 'client', '--example', 'scan_client_names')
    $line = Get-Content $log | Select-String '^== elsewhere: (\d+)' | Select-Object -First 1
    if (-not $line) { return @{ Ok = $false; Note = "(no result, exit $code)" } }
    $n = [int]$line.Matches[0].Groups[1].Value
    @{ Ok = ($code -eq 0 -and $n -eq 0); Note = "($n client names written as text elsewhere)" }
}

if (-not $SkipRelease) {
    Step 'release build + scan' (Join-Path $logs 'check-release-build.log') {
        param($log)
        $code = Run $log @('powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $PSScriptRoot 'release-build.ps1'), '-Junction', $Junction)
        @{ Ok = ($code -eq 0); Note = "(exit $code)" }
    }
    Step 'release archive + scan' (Join-Path $logs 'check-release-archive.log') {
        param($log)
        $cmd = @('powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $PSScriptRoot 'release-archive.ps1'), '-SkipBuild', '-Junction', $Junction)
        if ($Version) { $cmd += @('-Version', $Version) }
        $code = Run $log $cmd
        $line = Get-Content $log | Select-String 'release-archive: PASS, (.*)' | Select-Object -Last 1
        @{ Ok = ($code -eq 0); Note = $(if ($line) { "($($line.Matches[0].Groups[1].Value))" } else { "(exit $code)" }) }
    }
}

Write-Host "`n=== local check summary"
$all = $true
foreach ($k in $results.Keys) {
    $r = $results[$k]
    if (-not $r.Ok) { $all = $false }
    Write-Host ("  {0,-24} {1}  {2,5}s  {3}" -f $k, $(if ($r.Ok) { 'PASS' } else { 'FAIL' }), $r.Secs, $r.Note)
}
Write-Host $(if ($all) { 'local check: PASS' } else { 'local check: FAIL (logs in logs\check-*.log)' })
if ($all) { exit 0 } else { exit 1 }
