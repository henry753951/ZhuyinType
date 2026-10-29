# Builds ZhuyinType-Windows-x64.zip from finished build trees. Run from the repository root on Windows:
#   platforms\windows\package.ps1 -HostBuild .build-qt\Release -TsfBuild .build-windows-tsf\Release -Out dist
param([Parameter(Mandatory)][string]$HostBuild, [Parameter(Mandatory)][string]$TsfBuild, [string]$Out='dist')
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$stage=Join-Path ([IO.Path]::GetTempPath()) ('zt-package-'+[guid]::NewGuid())
$pkg=Join-Path $stage 'ZhuyinType-Windows-x64'
New-Item -ItemType Directory -Force "$pkg\app","$pkg\tsf","$pkg\app\data" | Out-Null
# The host exe with the Qt runtime that windeployqt placed next to it, and the engine library.
Copy-Item "$HostBuild\*" "$pkg\app" -Recurse -Force -Exclude '*.lib','*.exp','*.pdb','*.ilk','CMakeFiles','*.cmake','*.vcxproj*','*.obj','*.rc'
Copy-Item "$root\data\build\*" "$pkg\app\data" -Recurse -Force
Copy-Item "$TsfBuild\zhuyintype-tsf.dll","$TsfBuild\zt-tsf-register.exe" "$pkg\tsf"
Copy-Item "$root\platforms\windows\src\logo.ico" "$pkg\tsf"
Copy-Item "$PSScriptRoot\package\install.ps1","$PSScriptRoot\user-profile.ps1" $pkg
Copy-Item "$root\LICENSE","$root\NOTICE" $pkg
New-Item -ItemType Directory -Force $Out | Out-Null
$zip=Join-Path (Resolve-Path $Out) 'ZhuyinType-Windows-x64.zip'
if (Test-Path $zip) { Remove-Item $zip }
# bsdtar (built into Windows 10 and later) writes the zip; Compress-Archive rejects some file timestamps.
tar.exe -a -c -f $zip -C $pkg .
if ($LASTEXITCODE) { throw "zip failed" }
(Get-FileHash $zip -Algorithm SHA256).Hash.ToLower() + '  ZhuyinType-Windows-x64.zip' | Set-Content "$zip.sha256"
Write-Output $zip
