param(
    [Parameter(Mandatory = $true)][string]$Zig,
    [Parameter(Mandatory = $true)][string]$Prefix
)

# US-0208 native ABI proof, separate from the production application build.
# Inputs are an already installed toolchain and upstream static-library prefix.
# This script deliberately performs no downloads or system installation.
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))

function Resolve-ProjectPath([string]$Path) {
    $resolved = (Resolve-Path -LiteralPath $Path).Path
    $boundary = $root + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Native probe inputs must stay inside the project: $resolved"
    }
    return $resolved
}

$zigPath = Resolve-ProjectPath $Zig
$prefixPath = Resolve-ProjectPath $Prefix
$version = & $zigPath version
if ($LASTEXITCODE -ne 0 -or $version.Trim() -ne '0.17.1') {
    throw 'The libghostty-vt probe requires the pinned Zig 0.17.1 toolchain.'
}
$header = Join-Path $prefixPath 'include/ghostty/vt.h'
$library = Join-Path $prefixPath 'lib/ghostty-vt-static.lib'
if (-not (Test-Path -LiteralPath $header -PathType Leaf) -or
    -not (Test-Path -LiteralPath $library -PathType Leaf)) {
    throw 'Prefix must contain matching upstream headers and ghostty-vt-static.lib.'
}

$output = Join-Path $root '.tmp/ghostty-migration/probe'
New-Item -ItemType Directory -Force $output | Out-Null
$env:TMP = $output
$env:TEMP = $output
$env:TMPDIR = $output
$env:ZIG_GLOBAL_CACHE_DIR = Join-Path $output 'zig-cache'
$binary = Join-Path $output 'ghostty-vt-probe.exe'
& $zigPath cc -target x86_64-windows-msvc -std=c11 -Wall -Wextra -Werror `
    -DGHOSTTY_STATIC -I (Join-Path $prefixPath 'include') `
    (Join-Path $PSScriptRoot 'probe.c') $library -lntdll -lkernel32 -o $binary
if ($LASTEXITCODE -ne 0) { throw 'libghostty-vt C ABI probe compilation/linking failed.' }
& $binary
if ($LASTEXITCODE -ne 0) { throw 'libghostty-vt C ABI probe failed.' }

# Published viewports and gesture ownership use the actual crate graph. A
# rustc-only module copy no longer represents that boundary faithfully.
$env:GHOSTTY_VT_PREFIX = $prefixPath
if (-not $env:CARGO_HOME) { $env:CARGO_HOME = Join-Path $root '.cargo-home' }
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $root 'target' }
& cargo test --manifest-path (Join-Path $root 'Cargo.toml') `
    -p oneterm-terminal --features ghostty-native -- --test-threads=1
if ($LASTEXITCODE -ne 0) { throw 'libghostty-vt Rust boundary/ownership tests failed.' }
