# Runs .github/workflows/rust.yml locally on Windows, for checking a commit before it is pushed.
#
# The commit is cloned next to this repository with LF line endings and without
# Cargo.lock, so dependencies resolve fresh as they do in CI. The steps run with
# CI's toolchains into a fresh target inside the clone. The clone and its target
# are deleted afterwards; the logs are kept. Unlike CI, every step runs even
# after a failure, so one run shows every failing step. Keep the steps in sync
# with rust.yml.
#
# usage: pwsh -File tools/ci-local.ps1 [-Commit <rev>] [-LogDir <dir>] [-Push]
#   -Push  push the commit to origin/master afterwards, only if every step
#          passed and the push is a fast-forward.
param(
    [string]$Commit = 'HEAD',
    [string]$LogDir,
    [switch]$Push
)
$ErrorActionPreference = 'Stop'

$repo = Split-Path $PSScriptRoot
$sha = git -C $repo rev-parse --verify "$Commit^{commit}"
$work = Join-Path (Split-Path $repo) "umya-ci-$($sha.Substring(0, 8))"
if (-not $LogDir) { $LogDir = Join-Path ([IO.Path]::GetTempPath()) "umya-ci-$($sha.Substring(0, 8))" }
New-Item -ItemType Directory -Force $LogDir | Out-Null

# rust.yml's HEAVY: the test step skips the heavy chart conversion tests and the
# chart-conversion step, rust.yml's job run by hand, runs only them. Both run here.
$heavy = @(
    'chart_conversion::admitted::every_admitted_conversion_matches_excel'
    'chart_conversion::admitted::every_guarded_measurement_is_refused_with_its_listed_cause'
    'chart_conversion::classification::every_before_part_classifies_as_its_subtype'
    'chart_conversion::refusals::series_count_refusals_follow_excel_and_leave_the_chart_alone'
    'chart_conversion::umya_round_trip_keeps_every_before_part'
)
$skips = $heavy | ForEach-Object { '--skip'; $_ }
$steps = [ordered]@{
    toolchain = {
        rustup toolchain install 1.98.0 --profile minimal -c clippy --no-self-update &&
        rustup toolchain install nightly-2026-10-01 --profile minimal -c rustfmt --no-self-update
    }
    fmt       = { cargo +nightly-2026-10-01 fmt --all -- --check }
    clippy    = { cargo +1.98.0 clippy --features serde -- -D warnings }
    build     = { cargo +1.98.0 build --features serde }
    test      = { cargo +1.98.0 test --features serde -- $skips }
    'chart-conversion' = {
        $env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS = 'true'
        $env:CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS = 'true'
        try {
            $list = cargo +1.98.0 test --release --features serde --test integration -- $heavy --list
            if ($LASTEXITCODE) { return }
            $tests = @($list | Where-Object { $_ -match ': test$' })
            "$($tests.Count) chart conversion tests"
            # A filter that matches no test would pass with nothing run: fail instead.
            $unmatched = @($heavy | Where-Object { $filter = $_; -not ($tests | Where-Object { $_.Contains($filter) }) })
            if ($unmatched) { "no test matches $($unmatched -join ', ')"; $global:LASTEXITCODE = 1; return }
            cargo +1.98.0 test --release --features serde --test integration -- $heavy
        }
        finally {
            Remove-Item Env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS, Env:CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS
        }
    }
}

$failed = @()
Push-Location (Split-Path $repo)
try {
    # A run killed from outside skips the cleanup below and leaves its clone.
    if (Test-Path $work) { Remove-Item -Recurse -Force $work }
    git clone -q --no-local --no-checkout $repo $work
    if ($LASTEXITCODE) { throw "clone of $repo failed" }
    git -C $work -c core.autocrlf=false checkout -q --detach $sha
    if ($LASTEXITCODE) { throw "checkout of $sha failed" }
    $env:CARGO_TARGET_DIR = Join-Path $work 'target'
    Set-Location $work
    foreach ($name in $steps.Keys) {
        & $steps[$name] *> (Join-Path $LogDir "$name.log")
        if ($LASTEXITCODE) { $failed += $name; Write-Output "$name FAILED" } else { Write-Output "$name ok" }
    }
}
finally {
    Pop-Location
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
if (Test-Path $work) { throw "could not delete $work" }

if ($failed) {
    Write-Output "failed: $($failed -join ', '); logs in $LogDir"
    exit 1
}
Write-Output "all steps passed for $sha; logs in $LogDir"
if ($Push) {
    git -C $repo fetch -q origin
    git -C $repo merge-base --is-ancestor origin/master $sha
    if ($LASTEXITCODE) { Write-Output 'origin/master has moved: rebase first'; exit 1 }
    git -C $repo push origin "${sha}:refs/heads/master"
    exit $LASTEXITCODE
}
