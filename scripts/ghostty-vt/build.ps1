param(
    [Parameter(Mandatory = $true)][string]$Zig,
    [Parameter(Mandatory = $true)][string]$SourceArchive
)

# Build only the pinned upstream library. Never install or download tools here.
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$revision = 'fbb6dbe7ea5c1ce6df14ef82e00779a568cdb2a2'
$archiveHash = '2ff4f0aaf595bc07d36b4a08cf3e85a52581e3be792b295ae34a5db481197f99'
$boundary = $root + [IO.Path]::DirectorySeparatorChar
$zigPath = (Resolve-Path -LiteralPath $Zig).Path
$archive = (Resolve-Path -LiteralPath $SourceArchive).Path
foreach ($path in @($zigPath, $archive)) {
    if (-not $path.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Native build inputs must stay inside the project: $path"
    }
}
$version = & $zigPath version
if ($LASTEXITCODE -ne 0 -or $version.Trim() -ne '0.17.1') {
    throw 'The pinned Ghostty build requires Zig 0.17.1.'
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $archiveHash) {
    throw 'Pinned Ghostty source archive checksum mismatch.'
}

$work = Join-Path $root '.tmp/ghostty-migration/native-build'
$prefix = Join-Path $root '.tmp/ghostty-migration/install'
New-Item -ItemType Directory -Force $work | Out-Null
& tar -xf $archive -C $work
if ($LASTEXITCODE -ne 0) { throw 'Pinned Ghostty source extraction failed.' }
$source = Join-Path $work "ghostty-$revision"

# Keep Zig dependencies, compilation caches and temporary files in the project.
$variables = @('TMP', 'TEMP', 'TMPDIR', 'ZIG_GLOBAL_CACHE_DIR', 'ZIG_LOCAL_CACHE_DIR')
$previous = @{}
foreach ($name in $variables) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    $env:TMP = $work
    $env:TEMP = $work
    $env:TMPDIR = $work
    $env:ZIG_GLOBAL_CACHE_DIR = Join-Path $work 'zig-global-cache'
    $env:ZIG_LOCAL_CACHE_DIR = Join-Path $work 'zig-local-cache'
    Push-Location $source
    try {
        # SIMD is disabled for this ABI-first proof to avoid unrelated C++ tools.
        # Explicit versions prevent detecting the enclosing OneTerm git checkout.
        & $zigPath build -Demit-lib-vt -Dtarget=x86_64-windows-msvc `
            -Doptimize=ReleaseFast -Dsimd=false -j2 `
            '-Dversion-string=1.4.1-dev+fbb6dbe7' `
            '-Dlib-version-string=1.1.0-dev+fbb6dbe7' --prefix $prefix
        if ($LASTEXITCODE -ne 0) { throw 'Pinned libghostty-vt native build failed.' }
    } finally {
        Pop-Location
    }
    $library = Join-Path $prefix 'lib/ghostty-vt-static.lib'
    if (-not (Test-Path -LiteralPath $library -PathType Leaf)) {
        throw 'Upstream build did not install the expected static library.'
    }
    @{
        revision = $revision
        source_sha256 = $archiveHash
        zig = '0.17.1'
        target = 'x86_64-windows-msvc'
        simd = $false
        library_sha256 = (Get-FileHash -LiteralPath $library -Algorithm SHA256).Hash.ToLowerInvariant()
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $prefix 'oneterm-build.json') -Encoding utf8
    Set-Content -LiteralPath (Join-Path $prefix 'oneterm-revision.txt') -Value $revision -Encoding ascii
    Write-Output "Pinned libghostty-vt installed at $prefix"
} finally {
    foreach ($name in $variables) {
        [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process')
    }
}
