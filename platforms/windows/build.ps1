param([string]$BuildDir='.build-windows-tsf', [switch]$Install, [switch]$Machine, [string]$HostExe)
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $vs=if ($env:ZT_BUILD_TOOLS) { $env:ZT_BUILD_TOOLS } else { 'C:\BuildTools' }
    Import-Module "$vs\Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
    Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64'
    $env:PATH="$vs\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;$env:PATH"
}
Push-Location $root
try {
    & cmake -S platforms/windows -B $BuildDir -A x64 -DBUILD_TESTING=ON
    if ($LASTEXITCODE) { throw 'CMake 設定失敗' }
    & cmake --build $BuildDir --config Release --parallel 4
    if ($LASTEXITCODE) { throw '建置失敗' }
    & ctest --test-dir $BuildDir -C Release --output-on-failure
    if ($LASTEXITCODE) { throw '測試失敗' }
    if ($Install) {
        # Windows keeps in-process TIP DLLs mapped until each client exits.
        $version=(Get-FileHash "$BuildDir\Release\zhuyintype-tsf.dll" -Algorithm SHA256).Hash.Substring(0,16)
        $destination=Join-Path $env:LOCALAPPDATA "ZhuyinType\Tsf\$version"
        New-Item -ItemType Directory -Force $destination | Out-Null
        foreach ($name in @('zhuyintype-tsf.dll','zt-tsf-register.exe')) {
            $source=Join-Path "$BuildDir\Release" $name
            $target=Join-Path $destination $name
            if ((Test-Path $target) -and (Get-FileHash $source).Hash -eq (Get-FileHash $target).Hash) { continue }
            Copy-Item $source $target
        }
        $arguments=@('install',"$destination\zhuyintype-tsf.dll")
        # The language list shows the DLL's own monochrome logo (src/logo.ico), not the colour app icon.
        if ($Machine) { $arguments+='--machine' }
        & "$destination\zt-tsf-register.exe" @arguments
        if ($LASTEXITCODE) { throw '註冊失敗' }
        & "$PSScriptRoot\user-profile.ps1"
    }
} finally { Pop-Location }
