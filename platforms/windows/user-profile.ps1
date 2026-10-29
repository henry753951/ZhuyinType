param([switch]$Remove)
$ErrorActionPreference='Stop'
$tip='0404:{25BC3548-B611-4F92-91D0-C9291D427EA8}{E8207CF3-7CE3-42AE-9B18-453284A73901}'
$languages=Get-WinUserLanguageList
$target=$languages | Where-Object { $_.LanguageTag -in @('zh-Hant-TW','zh-TW') } | Select-Object -First 1
if ($Remove) {
    if ($target -and $target.InputMethodTips.Contains($tip)) {
        $target.InputMethodTips.Remove($tip) | Out-Null
        Set-WinUserLanguageList $languages -Force
    }
} else {
    if (-not $target) {
        $addition=New-WinUserLanguageList 'zh-Hant-TW'
        $target=$addition[0]
        $languages.Add($target)
    }
    if (-not $target.InputMethodTips.Contains($tip)) {
        $target.InputMethodTips.Add($tip)
        Set-WinUserLanguageList $languages -Force
    }
}
