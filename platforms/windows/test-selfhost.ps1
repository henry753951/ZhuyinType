param([string]$HostBuildDir='.build-qt', [string]$QtRoot='C:\Qt\6.10.2\msvc2022_64')
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$evidence=Join-Path $root '.build-windows-tsf\evidence'
$env:PATH="$QtRoot\bin;$env:PATH"
$env:QT_PLUGIN_PATH=Join-Path $QtRoot 'plugins'
$env:QML_IMPORT_PATH=Join-Path $QtRoot 'qml'
$env:QT_QUICK_BACKEND='software'
$process=Start-Process "$root\.build-windows-tsf\Release\zt-qt-selfhost.exe" -Wait -PassThru `
    -ArgumentList "`"$root\$HostBuildDir\Release\zhuyin.dll`"","`"$root\data\build\lexicon.tsv`"" `
    -RedirectStandardOutput "$evidence\selfhost.txt" -RedirectStandardError "$evidence\selfhost.err"
Set-Content "$evidence\selfhost.exit" $process.ExitCode
exit $process.ExitCode
