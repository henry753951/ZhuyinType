param([ValidateSet('broker','unavailable','interactive','popup','stock')][string]$Mode='broker')
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$build=Join-Path $root '.build-windows-tsf'
$evidence=Join-Path $build 'evidence'
New-Item -ItemType Directory -Force $evidence | Out-Null
$arguments=if ($Mode -eq 'broker') { @() } else { @("--$Mode") }
$options=@{FilePath="$build\Release\zt-tsf-integration.exe"; Wait=$true; PassThru=$true;
    RedirectStandardOutput="$evidence\native-$Mode.txt"; RedirectStandardError="$evidence\native-$Mode.err"}
if ($arguments.Count) { $options.ArgumentList=$arguments }
if ($Mode -eq 'stock') { $options.FilePath="$build\Release\zt-stock-control.exe"; $options.Remove('ArgumentList') }
$process=Start-Process @options
$result=$process.ExitCode
Set-Content "$evidence\native-$Mode.exit" $result
exit $result
