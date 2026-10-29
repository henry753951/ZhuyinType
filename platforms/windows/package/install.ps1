# ZhuyinType for Windows: install from this extracted release folder, or pass -Uninstall.
# Copies the program to %LOCALAPPDATA%\ZhuyinType, registers the input method for the current user, starts the service at
# sign-in and adds a Start menu shortcut. Other input methods and languages are left as they are.
param([switch]$Uninstall)
$ErrorActionPreference='Stop'
$here=$PSScriptRoot
$root=Join-Path $env:LOCALAPPDATA 'ZhuyinType'
$app=Join-Path $root 'app'
$run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$entry='ZhuyinTypeInputHost'
$shortcut=Join-Path ([Environment]::GetFolderPath('Programs')) 'ZhuyinType Settings.lnk'
$tsfKey='HKCU:\Software\Classes\CLSID\{25BC3548-B611-4F92-91D0-C9291D427EA8}\InprocServer32'

function Stop-Service-Process {
    Get-CimInstance Win32_Process -Filter "Name='zhuyintype-desktop.exe'" | Where-Object { $_.CommandLine -like '*--ime-host*' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
}

if ($Uninstall) {
    if (Test-Path $tsfKey) {
        $dll=(Get-Item $tsfKey).GetValue('')
        $register=Join-Path (Split-Path $dll -Parent) 'zt-tsf-register.exe'
        if (Test-Path $register) { & $register uninstall; if ($LASTEXITCODE) { throw 'TSF 移除失敗' } }
    }
    & "$here\user-profile.ps1" -Remove
    Remove-ItemProperty $run -Name $entry -ErrorAction SilentlyContinue
    Remove-Item $shortcut -ErrorAction SilentlyContinue
    Stop-Service-Process
    Remove-Item $app -Recurse -Force -ErrorAction SilentlyContinue
    Write-Output 'ZhuyinType 已移除。開啟中的應用程式結束後，輸入法 DLL 會在那時卸載。'
    exit 0
}

# Replace the program. Running copies are stopped first; the TSF DLL lives in a hashed folder so an update never
# overwrites a DLL that an application still has loaded.
Stop-Service-Process
Remove-Item $app -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $app | Out-Null
Copy-Item (Join-Path $here 'app\*') $app -Recurse -Force
$dll=Join-Path $here 'tsf\zhuyintype-tsf.dll'
$version=(Get-FileHash $dll -Algorithm SHA256).Hash.Substring(0,16)
$tsfDirectory=Join-Path $root "Tsf\$version"
New-Item -ItemType Directory -Force $tsfDirectory | Out-Null
# An identical file already in place is kept: a DLL that an application has loaded cannot be rewritten.
foreach ($name in @('zhuyintype-tsf.dll','zt-tsf-register.exe')) {
    $source=Join-Path $here "tsf\$name"
    $target=Join-Path $tsfDirectory $name
    if (-not ((Test-Path $target) -and (Get-FileHash $source).Hash -eq (Get-FileHash $target).Hash)) { Copy-Item $source $target -Force }
}
& "$tsfDirectory\zt-tsf-register.exe" install "$tsfDirectory\zhuyintype-tsf.dll"
if ($LASTEXITCODE) { throw 'TSF 註冊失敗' }
& "$here\user-profile.ps1"

$exe=Join-Path $app 'zhuyintype-desktop.exe'
$launcher=Join-Path $root 'Start-InputHost.ps1'
$lines=@(
    '$ErrorActionPreference=''Stop''',
    "Start-Process -FilePath '$($exe.Replace("'","''"))' -WorkingDirectory '$($app.Replace("'","''"))' -ArgumentList @('--ime-host','--lexicon','$((Join-Path $app 'data\lexicon.tsv').Replace("'","''"))')"
)
Set-Content -Path $launcher -Value $lines -Encoding UTF8
if (-not (Test-Path $run)) { New-Item $run | Out-Null }
$command='"'+(Join-Path $PSHOME 'powershell.exe')+'" -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+$launcher+'"'
New-ItemProperty $run -Name $entry -PropertyType String -Value $command -Force | Out-Null
$link=(New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
$link.TargetPath=$exe
$link.WorkingDirectory=$app
$link.Description='Open ZhuyinType settings (開啟 ZhuyinType 設定)'
$link.Save()
& "$PSHOME\powershell.exe" -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File $launcher
Write-Output '已安裝。用 Win＋空白鍵選取「ZhuyinType 注音」；設定可從工作列右鍵選單或開始功能表的「ZhuyinType Settings」開啟。'
