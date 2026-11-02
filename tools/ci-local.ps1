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

$steps = [ordered]@{
    toolchain = {
        rustup toolchain install 1.98.0 --profile minimal -c clippy --no-self-update &&
        rustup toolchain install nightly-2026-10-01 --profile minimal -c rustfmt --no-self-update
    }
    fmt       = { cargo +nightly-2026-10-01 fmt --all -- --check }
    clippy    = { cargo +1.98.0 clippy --features serde -- -D warnings }
    build     = { cargo +1.98.0 build --features serde }
    test      = { cargo +1.98.0 test --features serde }
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
