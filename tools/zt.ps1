# Windows entry point for the same uv-managed CLI used by tools/zt.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$uv = $env:ZT_UV
if (-not $uv) {
    $command = Get-Command uv -ErrorAction SilentlyContinue
    if ($command) { $uv = $command.Source }
}
if (-not $uv) {
    $candidate = Join-Path $env:USERPROFILE '.local\bin\uv.exe'
    if (Test-Path $candidate -PathType Leaf) { $uv = $candidate }
}
if (-not $uv -or -not (Get-Command $uv -ErrorAction SilentlyContinue)) {
    throw 'zt: uv not found. Install uv before using the Windows tools.'
}
$previousPythonPath = $env:PYTHONPATH
$previousUv = $env:ZT_UV
try {
    $source = Join-Path $root 'ml\src'
    $env:PYTHONPATH = if ($previousPythonPath) { "$source;$previousPythonPath" } else { $source }
    $env:ZT_UV = $uv
    # Windows PowerShell 5 treats redirected native stderr as ErrorRecords.
    # Cargo writes normal progress there; the process exit code determines success.
    $previousErrorAction = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & $uv run --quiet --project (Join-Path $root 'ml') python -m zhuyintype_ml.cli @args
        $status = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorAction
    }
} finally {
    $env:PYTHONPATH = $previousPythonPath
    $env:ZT_UV = $previousUv
}
exit $status
