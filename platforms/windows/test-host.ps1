param([string]$BuildDir='.build-qt')
# Run in the test VM's interactive user session, after the shared Qt build is deployed.
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$evidence=Join-Path $root '.build-windows-tsf\evidence'
New-Item -ItemType Directory -Force $evidence | Out-Null
$env:QT_PLUGIN_PATH='C:\Qt\6.10.2\msvc2022_64\plugins'
$env:QT_QUICK_BACKEND='software'
$exe=Join-Path $root "$BuildDir\Release\zhuyintype-desktop.exe"
$process=Start-Process $exe -ArgumentList @('--ime-host','--lexicon',"`"$root\data\build\lexicon.tsv`"") -WorkingDirectory (Split-Path $exe) -PassThru `
    -RedirectStandardOutput "$evidence\host.txt" -RedirectStandardError "$evidence\host.err"
Set-Content "$evidence\host.pid" $process.Id
