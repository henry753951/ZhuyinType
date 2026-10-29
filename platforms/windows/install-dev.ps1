param([string]$HostExe, [string]$Lexicon, [switch]$SoftwareRendering, [switch]$Uninstall)
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$entry='ZhuyinTypeInputHost'
if ($Uninstall) {
    & "$PSScriptRoot\user-profile.ps1" -Remove
    $key='HKCU:\Software\Classes\CLSID\{25BC3548-B611-4F92-91D0-C9291D427EA8}\InprocServer32'
    if (Test-Path $key) {
        $dll=(Get-Item $key).GetValue('')
        $register=Join-Path (Split-Path $dll -Parent) 'zt-tsf-register.exe'
        & $register uninstall
        if ($LASTEXITCODE) { throw '移除 TSF 註冊失敗' }
    }
    Remove-ItemProperty $run -Name $entry -ErrorAction SilentlyContinue
    Write-Output '已移除輸入法註冊與登入啟動項目。開啟中的應用程式結束後會卸載 DLL。'
    exit 0
}
if (-not $HostExe) { $HostExe=Join-Path $root '.build-qt\Release\zhuyintype-desktop.exe' }
$HostExe=(Resolve-Path $HostExe).Path
if (-not $Lexicon) { $Lexicon=Join-Path $root 'data\build\lexicon.tsv' }
$Lexicon=(Resolve-Path $Lexicon).Path
& "$PSScriptRoot\build.ps1" -Install -HostExe $HostExe
if ($LASTEXITCODE) { throw '原生輸入法安裝失敗，未登記登入啟動。' }
$directory=Join-Path $env:LOCALAPPDATA 'ZhuyinType'
New-Item -ItemType Directory -Force $directory | Out-Null
$launcher=Join-Path $directory 'Start-InputHost.ps1'
$exeLiteral=$HostExe.Replace("'","''")
$dataLiteral=$Lexicon.Replace("'","''")
$workingLiteral=(Split-Path $HostExe -Parent).Replace("'","''")
$lines=@('$ErrorActionPreference=''Stop''')
if ($SoftwareRendering) { $lines+='$env:QT_QUICK_BACKEND=''software''' }
$lines+="Start-Process -FilePath '$exeLiteral' -WorkingDirectory '$workingLiteral' -ArgumentList @('--ime-host','--lexicon','`"$dataLiteral`"')"
Set-Content -Path $launcher -Value $lines -Encoding UTF8
if (-not (Test-Path $run)) { New-Item $run | Out-Null }
$powershell=Join-Path $PSHOME 'powershell.exe'
$command='"'+$powershell+'" -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+$launcher+'"'
New-ItemProperty $run -Name $entry -PropertyType String -Value $command -Force | Out-Null
Write-Output '已登記輸入法與目前使用者的登入啟動項目。請使用 Win+空白鍵選取 ZhuyinType 注音。'
