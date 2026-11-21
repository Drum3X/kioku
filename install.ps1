# kioku bootstrap for Windows: makes sure this checkout has its compiled CLI (bin\kioku.exe) for
# the version in VERSION, then runs `bin\kioku.exe install` with the same arguments.
#
#   powershell -ExecutionPolicy Bypass -File install.ps1 [--vault PATH] [--uninstall] [--no-codex] ...
#
# The binary comes from, in order:
#   1. bin\kioku.exe already there with the right version;
#   2. the GitHub release for this version (sha256 checked);
#   3. a build from source with cargo ($env:KIOKU_BUILD = 1 forces this).
$ErrorActionPreference = 'Stop'

function Die($msg) { [Console]::Error.WriteLine("kioku: $msg"); exit 1 }

$Sys = $PSScriptRoot
$Bin = Join-Path $Sys 'bin\kioku.exe'
$Ver = (Get-Content (Join-Path $Sys 'VERSION') -Raw).Trim()
$Repo = if ($env:KIOKU_REPO) { $env:KIOKU_REPO } else { 'Drum3X/kioku' }

function Current {
    if (-not (Test-Path $Bin)) { return $false }
    try { return ((& $Bin version 2>$null) -join "`n") -match "^$([regex]::Escape($Ver)) " } catch { return $false }
}

function Download {
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($arch -ne 'AMD64') { return $false }
    $t = 'x86_64-pc-windows-msvc'
    $url = "https://github.com/$Repo/releases/download/v$Ver"
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("kioku-" + [guid]::NewGuid())
    New-Item -ItemType Directory $tmp | Out-Null
    try {
        Invoke-WebRequest "$url/kioku-$t.exe" -OutFile "$tmp\kioku.exe" -UseBasicParsing
        Invoke-WebRequest "$url/SHA256SUMS" -OutFile "$tmp\SHA256SUMS" -UseBasicParsing
    } catch { Remove-Item -Recurse -Force $tmp; return $false }
    $want = (Get-Content "$tmp\SHA256SUMS" | Where-Object { $_ -match "\s\*?kioku-$t\.exe$" } | ForEach-Object { ($_ -split '\s+')[0] }) | Select-Object -First 1
    $got = (Get-FileHash "$tmp\kioku.exe" -Algorithm SHA256).Hash.ToLower()
    if (-not $want -or $want.ToLower() -ne $got) { Remove-Item -Recurse -Force $tmp; Die "checksum mismatch for kioku-$t.exe from $url" }
    if (Test-Path $Bin) { Remove-Item -Force $Bin }
    Move-Item "$tmp\kioku.exe" $Bin
    Remove-Item -Recurse -Force $tmp
    Write-Host "kioku $Ver downloaded ($t)"
    return $true
}

function Build {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { return $false }
    Write-Host "building kioku $Ver from source (cargo build --release, with hybrid search) ..."
    Push-Location $Sys
    try {
        cargo build --release --locked --features semantic-bundled
        if ($LASTEXITCODE -ne 0) {
            Write-Host "kioku: building with hybrid search failed; building without it"
            cargo build --release --locked
            if ($LASTEXITCODE -ne 0) { Die 'cargo build failed' }
        }
    } finally { Pop-Location }
    Copy-Item -Force (Join-Path $Sys 'target\release\kioku.exe') $Bin
    return $true
}

New-Item -ItemType Directory -Force (Join-Path $Sys 'bin') | Out-Null
if ($env:KIOKU_BUILD) {
    if (-not (Build)) { Die 'KIOKU_BUILD is set but cargo was not found (https://rustup.rs)' }
} elseif (-not (Current)) {
    if (-not (Download)) {
        if (-not (Build)) { Die "no kioku binary for Windows ($env:PROCESSOR_ARCHITECTURE): no release download for v$Ver and no cargo to build it (install Rust from https://rustup.rs, then run this again)" }
    }
}
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { Die "'git' is required (Git for Windows: https://git-scm.com/download/win)" }
& $Bin install @args
exit $LASTEXITCODE
