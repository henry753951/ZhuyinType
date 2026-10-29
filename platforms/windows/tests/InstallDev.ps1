param([Parameter(Mandatory=$true)][string]$HostExe)
$ErrorActionPreference='Stop'
$scripts=Split-Path $PSScriptRoot -Parent
$run='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$entry='ZhuyinTypeInputHost'
$tip='0404:{25BC3548-B611-4F92-91D0-C9291D427EA8}{E8207CF3-7CE3-42AE-9B18-453284A73901}'
function OtherLanguages {
    @(Get-WinUserLanguageList | ForEach-Object {
        [ordered]@{ LanguageTag=$_.LanguageTag; Tips=@($_.InputMethodTips | Where-Object { $_ -ne $tip }) }
    }) | ConvertTo-Json -Depth 5 -Compress
}
function RunValues([switch]$All) {
    $values=[ordered]@{}
    if (Test-Path $run) {
        $key=Get-Item $run
        foreach ($name in ($key.GetValueNames() | Sort-Object)) {
            if ($All -or $name -ne $entry) { $values[$name]=$key.GetValue($name) }
        }
    }
    ConvertTo-Json -InputObject $values -Depth 5 -Compress
}
function Check($condition,$name) {
    if (-not $condition) { throw $name }
    Write-Output "PASS $name"
}
$beforeLanguages=OtherLanguages
$beforeRun=RunValues
$beforeDefault=Get-WinDefaultInputMethodOverride | ConvertTo-Json -Compress
& "$scripts\install-dev.ps1" -HostExe $HostExe -SoftwareRendering
Check ((OtherLanguages) -eq $beforeLanguages) 'installation preserves other languages and input methods'
Check ((RunValues) -eq $beforeRun) 'installation preserves other startup entries'
$launcher=Join-Path $env:LOCALAPPDATA 'ZhuyinType\Start-InputHost.ps1'
Check ((Get-Content $launcher -Raw).Contains($HostExe)) 'launcher uses explicit host path'
Check ((Get-Content $launcher -Raw).Contains('--lexicon')) 'launcher uses absolute lexicon'
& "$scripts\user-profile.ps1"
Check (@(Get-WinUserLanguageList | ForEach-Object { $_.InputMethodTips } | Where-Object { $_ -eq $tip }).Count -eq 1) 'profile addition is idempotent'
& "$scripts\user-profile.ps1" -Remove
Check ((OtherLanguages) -eq $beforeLanguages) 'profile removal preserves other input methods'
& "$scripts\user-profile.ps1"
$installedRun=RunValues -All
$failed=$false
try { & "$scripts\install-dev.ps1" -HostExe 'Z:\ZhuyinType-missing-test.exe' } catch { $failed=$true }
Check ($failed -and (RunValues -All) -eq $installedRun) 'invalid host leaves startup unchanged'
try {
    function global:cmake { $global:LASTEXITCODE=91 }
    $failed=$false
    try { & "$scripts\install-dev.ps1" -HostExe $HostExe } catch { $failed=$true }
    Check ($failed -and (RunValues -All) -eq $installedRun -and (OtherLanguages) -eq $beforeLanguages) 'build failure leaves profile and startup unchanged'
} finally { Remove-Item Function:\cmake -ErrorAction SilentlyContinue }
& "$scripts\install-dev.ps1" -Uninstall
Check ((OtherLanguages) -eq $beforeLanguages -and (RunValues) -eq $beforeRun) 'uninstall preserves other profiles and startup'
Check (-not (Test-Path 'HKCU:\Software\Classes\CLSID\{25BC3548-B611-4F92-91D0-C9291D427EA8}')) 'uninstall removes own COM class'
Check (-not (Get-Item $run).GetValue($entry)) 'uninstall removes own startup entry'
& "$scripts\install-dev.ps1" -HostExe $HostExe -SoftwareRendering
Check ((OtherLanguages) -eq $beforeLanguages -and (RunValues) -eq $beforeRun) 'final installation preserves other configuration'
Check ((Get-WinDefaultInputMethodOverride | ConvertTo-Json -Compress) -eq $beforeDefault) 'installation preserves default input override'
Write-Output 'PASS development installation and startup registration'
