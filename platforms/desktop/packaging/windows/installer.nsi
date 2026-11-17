; ZhuyinType for Windows: the installer and the uninstaller (NSIS 3, Modern UI 2, Traditional Chinese).
;
; Built by build.sh on the Linux host:
;   makensis -INPUTCHARSET UTF8 -DVERSION=0.5.3 -DSTAGE=DIR -DOUTFILE=FILE [-DBRANDING=DIR] [-DSIGN="cmd %1"] installer.nsi
; STAGE holds zhuyintype.exe, zhuyintype-tsf-x64.dll, zhuyintype-tsf-x86.dll, logo.ico, zhuyintype.ico, data/ and
; licenses/ (build-time paths use /, install-time paths \).  See README.md for what the installer does and why.

Unicode true
Target amd64-unicode
ManifestDPIAware true
ManifestSupportedOS Win10
RequestExecutionLevel admin
SetCompressor /SOLID lzma
SetCompressorDictSize 64

!ifndef VERSION | STAGE | OUTFILE
  !error "VERSION, STAGE and OUTFILE are required (build.sh passes them)"
!endif

; Code signing: build.sh passes SIGN when ZT_SIGN_COMMAND is set; %1 is the file to sign.  (TODO: a certificate.)
!ifdef SIGN
  !finalize '${SIGN}'
  !uninstfinalize '${SIGN}'
!endif

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WinVer.nsh"
!include "x64.nsh"
!include "StrFunc.nsh"
${StrStr}

!define PRODUCT "ZhuyinType"
!define PUBLISHER "ZhuyinType"
!define WEBSITE "https://github.com/Henry753951/ZhuyinType"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ZhuyinType"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
; Active Setup adds the input method to the list of every other user once, at their next sign-in.  Its version
; never changes, so an upgrade does not put it back for someone who removed it.
!define ACTIVE_SETUP_KEY "Software\Microsoft\Active Setup\Installed Components\{25BC3548-B611-4F92-91D0-C9291D427EA8}"
!define SHORTCUT "ZhuyinType 設定.lnk"
!define APP "zhuyintype.exe"
; Installs updates (zt_core::update): started on demand by the signed-in user's app, runs as LocalSystem.
!define UPDATE_SERVICE "ZhuyinTypeUpdate"
!define TSF64 "zhuyintype-tsf-x64.dll"
!define TSF32 "zhuyintype-tsf-x86.dll"

Name "${PRODUCT}"
Caption "${PRODUCT} ${VERSION} 安裝程式"
UninstallCaption "移除 ${PRODUCT}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\${PRODUCT}"
InstallDirRegKey HKLM "Software\${PRODUCT}" "InstallDir"
BrandingText "${PRODUCT} ${VERSION}"
ShowInstDetails nevershow
ShowUninstDetails nevershow

VIProductVersion "${VERSION}.0"
VIFileVersion "${VERSION}.0"
VIAddVersionKey /LANG=1028 "ProductName" "${PRODUCT}"
VIAddVersionKey /LANG=1028 "CompanyName" "${PUBLISHER}"
VIAddVersionKey /LANG=1028 "FileDescription" "${PRODUCT} 注音輸入法安裝程式"
VIAddVersionKey /LANG=1028 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1028 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1028 "LegalCopyright" "AGPL-3.0-only"

; ---------------------------------------------------------------------------------------------------------------
; Pages

!define MUI_ICON "${STAGE}/zhuyintype.ico"
!define MUI_UNICON "${STAGE}/zhuyintype.ico"
!ifdef BRANDING
  !define MUI_WELCOMEFINISHPAGE_BITMAP "${BRANDING}/welcome.bmp"
  !define MUI_UNWELCOMEFINISHPAGE_BITMAP "${BRANDING}/welcome.bmp"
  !define MUI_HEADERIMAGE
  !define MUI_HEADERIMAGE_RIGHT
  !define MUI_HEADERIMAGE_BITMAP "${BRANDING}/header.bmp"
  !define MUI_HEADERIMAGE_UNBITMAP "${BRANDING}/header.bmp"
!endif
!define MUI_ABORTWARNING
!define MUI_ABORTWARNING_TEXT "確定要結束 ${PRODUCT} 安裝程式嗎？"

!define MUI_WELCOMEPAGE_TITLE "安裝 ${PRODUCT} ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "ZhuyinType 是開源的注音輸入法。$\r$\n$\r$\n安裝程式會把 ZhuyinType 加入「繁體中文」的輸入法清單，並在您登入時自動啟動。您原本的輸入法與設定不會改變。$\r$\n$\r$\n如果已經安裝舊版，會直接更新；您的自訂詞與學習記錄都會保留。$\r$\n$\r$\n$_CLICK"
!insertmacro MUI_PAGE_WELCOME
!define MUI_LICENSEPAGE_TEXT_TOP "ZhuyinType 以 GNU AGPL 第 3 版授權，可以自由使用。"
!define MUI_LICENSEPAGE_BUTTON "下一步(&N) >"
!define MUI_LICENSEPAGE_TEXT_BOTTOM "按「下一步」繼續。"
!insertmacro MUI_PAGE_LICENSE "${STAGE}/licenses/LICENSE.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "ZhuyinType 已安裝"
!define MUI_FINISHPAGE_TEXT "按 Win＋空白鍵，選取「ZhuyinType 注音」就可以開始打字。$\r$\n$\r$\n設定可以從開始功能表的「ZhuyinType 設定」，或在工作列的「中／英」上按右鍵開啟。"
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "開啟 ZhuyinType"
!define MUI_FINISHPAGE_RUN_FUNCTION OpenApp
!insertmacro MUI_PAGE_FINISH

!define MUI_UNCONFIRMPAGE_TEXT_TOP "將從這台電腦移除 ZhuyinType。正在使用輸入法的程式結束或重新開機後，檔案才會完全刪除。"
!insertmacro MUI_UNPAGE_CONFIRM
!define MUI_COMPONENTSPAGE_TEXT_TOP "您的自訂詞、學習記錄與設定預設會保留，之後重新安裝可以繼續使用。"
!define MUI_COMPONENTSPAGE_TEXT_COMPLIST "選取要移除的項目："
!insertmacro MUI_UNPAGE_COMPONENTS
!insertmacro MUI_UNPAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "ZhuyinType 已移除"
!define MUI_FINISHPAGE_TEXT "ZhuyinType 已從這台電腦移除。$\r$\n$\r$\n仍在使用輸入法的程式結束後就不再載入它；剩下的檔案會在下次重新開機時刪除。"
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "TradChinese"
; The system UI font of Traditional Chinese Windows, instead of the language file's 新細明體.
SetFont /LANG=${LANG_TRADCHINESE} "Microsoft JhengHei UI" 9

; ---------------------------------------------------------------------------------------------------------------
; Helpers

; The sessions the app ran in before the installer stopped it ("1,3"; "all" when they could not be told apart), and
; the installer's own session.
Var AppSessions
Var InstallerSession
; Whether ZhuyinType was installed before (an upgrade), and whether the update service runs this installer (/UPDATE,
; as SYSTEM).
Var Upgrade
Var IsUpdate

Function RecordAppSessions
  System::Call 'kernel32::GetCurrentProcessId() i .r0'
  System::Call 'kernel32::ProcessIdToSessionId(i r0, *i .r1)'
  StrCpy $InstallerSession $1
  StrCpy $AppSessions ""
  ; Through a file: with its input redirected PowerShell may also print progress records (CLIXML) to the output.
  InitPluginsDir
  Delete "$PLUGINSDIR\sessions.txt"
  nsExec::Exec `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -InputFormat None -Command "$$ProgressPreference = 'SilentlyContinue'; (@(Get-Process zhuyintype -ErrorAction SilentlyContinue | ForEach-Object SessionId | Sort-Object -Unique) -join ',') | Set-Content -Encoding Ascii -LiteralPath '$PLUGINSDIR\sessions.txt'"`
  Pop $0
  ClearErrors
  FileOpen $3 "$PLUGINSDIR\sessions.txt" r
  ${If} $0 == 0
  ${AndIfNot} ${Errors}
    FileRead $3 $2
    FileClose $3
    ; Session numbers and commas only; anything else counts as not knowing.
    StrCpy $4 0
    ${Do}
      StrCpy $5 $2 1 $4
      ${If} $5 == ""
        ${Break}
      ${EndIf}
      ${StrStr} $6 "0123456789," $5
      ${If} $6 != ""
        StrCpy $AppSessions "$AppSessions$5"
      ${ElseIf} $5 != "$\r"
      ${AndIf} $5 != "$\n"
        StrCpy $AppSessions "unknown"
        ${Break}
      ${EndIf}
      IntOp $4 $4 + 1
    ${Loop}
  ${Else}
    StrCpy $AppSessions "unknown"
  ${EndIf}
  ${If} $AppSessions == "unknown"
    ; Without PowerShell: every signed-in session if the app runs anywhere.
    StrCpy $AppSessions ""
    nsExec::ExecToStack '"$SYSDIR\tasklist.exe" /NH /FO CSV /FI "IMAGENAME eq ${APP}"'
    Pop $0
    Pop $2
    StrCpy $3 $2 1
    ${If} $3 == '"'
      StrCpy $AppSessions "all"
    ${EndIf}
  ${EndIf}
FunctionEnd

; Ends the running app (the input method host) for every user: first politely, then for good, and waits (up to 10 s)
; until it has exited so that its files can be replaced.
!macro StopApp
  nsExec::Exec '"$SYSDIR\taskkill.exe" /IM ${APP}'
  Pop $0
  Sleep 1500
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM ${APP}'
  Pop $0
  StrCpy $R8 0
  ${Do}
    Sleep 250
    nsExec::ExecToStack '"$SYSDIR\tasklist.exe" /NH /FO CSV /FI "IMAGENAME eq ${APP}"'
    Pop $0
    Pop $1
    StrCpy $1 $1 1
    IntOp $R8 $R8 + 1
    ${If} $1 != '"'
    ${OrIf} $R8 >= 40
      ${Break}
    ${EndIf}
  ${Loop}
!macroend

; Moves a DLL that applications still have loaded out of the way: Windows lets a loaded DLL be renamed, not
; replaced.  Those applications keep the old copy until they exit; the next install or the next restart deletes it.
; Nothing needs a restart, so the finish page does not offer one.
!macro MoveAside FILE
  ${If} ${FileExists} "${FILE}"
    Delete "${FILE}"
    ${If} ${FileExists} "${FILE}"
      System::Call 'kernel32::GetTickCount() i .r9'
      Rename "${FILE}" "${FILE}.$9.old"
      Delete /REBOOTOK "${FILE}.$9.old"
      SetRebootFlag false
    ${EndIf}
  ${EndIf}
!macroend

; Leftovers of earlier upgrades whose applications have exited since.
!macro DeleteOldCopies
  FindFirst $0 $1 "$INSTDIR\*.old"
  ${DoWhile} $1 != ""
    Delete "$INSTDIR\$1"
    FindNext $0 $1
  ${Loop}
  FindClose $0
!macroend

; Starts the app as the signed-in user, not elevated like the installer: the broker's pipe must belong to that user.
; Explorer opens the file in the user's own session.
Function LaunchAsUser
  Exec '"$WINDIR\explorer.exe" "$INSTDIR\${APP}"'
FunctionEnd

; Whether this installer runs as the user signed in to its session ($0 = 1): an administrator who answered the UAC
; prompt.  A standard user who typed an administrator's name and password runs it as that administrator ($0 = 0), and
; Explorer would not start the app as them.
Function IsSessionUser
  StrCpy $0 1
  System::Call 'wtsapi32::WTSQuerySessionInformationW(p 0, i -1, i 5, *p .r1, *i .r2) i .r3'
  ${If} $3 == 0
    Return
  ${EndIf}
  System::Call 'kernel32::lstrcpynW(w .r4, p r1, i ${NSIS_MAX_STRLEN})'
  System::Call 'wtsapi32::WTSFreeMemory(p r1)'
  System::Call 'wtsapi32::WTSQuerySessionInformationW(p 0, i -1, i 7, *p .r1, *i .r2) i .r3'
  StrCpy $5 ""
  ${If} $3 != 0
    System::Call 'kernel32::lstrcpynW(w .r5, p r1, i ${NSIS_MAX_STRLEN})'
    System::Call 'wtsapi32::WTSFreeMemory(p r1)'
  ${EndIf}
  System::Call 'advapi32::GetUserNameW(w .r6, *i ${NSIS_MAX_STRLEN}) i .r3'
  ReadEnvStr $7 USERDOMAIN
  ${If} $3 == 0
    Return
  ${EndIf}
  ${If} $4 != $6
  ${OrIf} $5 != $7
    StrCpy $0 0
  ${EndIf}
FunctionEnd

; Has the update service start the app as each listed session's own user (installer.nsi writes the request under
; HKLM\Software\ZhuyinType; zt_core::update::windows::relaunch_sessions).  The service runs as SYSTEM, which can act
; for every signed-in user; this installer may run as another account than the person at the screen.  $0 is sc's exit
; code: 0 when the service started.  It is retried while an earlier run is still ending.
Function StartRelaunch
  StrCpy $R7 0
  ${Do}
    nsExec::Exec '"$SYSDIR\sc.exe" start ${UPDATE_SERVICE} relaunch'
    Pop $0
    IntOp $R7 $R7 + 1
    ${If} $0 == 0
    ${OrIf} $R7 >= 12
      ${Break}
    ${EndIf}
    Sleep 250
  ${Loop}
FunctionEnd

; The finish page's 開啟 ZhuyinType: the settings, in this session, as its user.
Function OpenApp
  Call IsSessionUser
  ${If} $0 == 1
    Call LaunchAsUser
    Return
  ${EndIf}
  WriteRegStr HKLM "Software\${PRODUCT}" "OpenSession" "$InstallerSession"
  Call StartRelaunch
  ${If} $0 != 0
    DeleteRegValue HKLM "Software\${PRODUCT}" "OpenSession"
    Call LaunchAsUser
  ${EndIf}
FunctionEnd

; Starts the app again in the background where it ran before the installer stopped it ($AppSessions).
Function RelaunchApp
  ${If} $AppSessions == ""
    Return
  ${EndIf}
  WriteRegStr HKLM "Software\${PRODUCT}" "RelaunchSessions" "$AppSessions"
  ${If} $IsUpdate == 1
    ; Already SYSTEM.
    Exec '"$INSTDIR\${APP}" --relaunch-sessions'
    Return
  ${EndIf}
  Call StartRelaunch
  ${If} $0 != 0
    DeleteRegValue HKLM "Software\${PRODUCT}" "RelaunchSessions"
    Call LaunchInBackgroundAsUser
  ${EndIf}
FunctionEnd

; The installation folder must be ZhuyinType's own: new, empty, or an earlier installation (or what an uninstall left
; until the next restart: text service DLLs that applications still had loaded).  The installer gives it
; to the administrators (the update service runs its program as SYSTEM; the text service runs inside every
; application) and the uninstaller deletes it, so a shared folder (C:\, Program Files, a user's folder) would be
; damaged.  $R0 = 1 when it may be used.
Function CheckInstallDir
  StrCpy $R0 0
  StrLen $0 $INSTDIR
  ${If} $0 <= 3
    Return
  ${EndIf}
  ${If} ${FileExists} "$INSTDIR\${APP}"
  ${OrIf} ${FileExists} "$INSTDIR\uninstall.exe"
  ${OrIf} ${FileExists} "$INSTDIR\zhuyintype-tsf-*"
  ${OrIfNot} ${FileExists} "$INSTDIR\*.*"
    StrCpy $R0 1
    Return
  ${EndIf}
  StrCpy $R0 1
  FindFirst $0 $1 "$INSTDIR\*.*"
  ${DoWhile} $1 != ""
    ${If} $1 != "."
    ${AndIf} $1 != ".."
      StrCpy $R0 0
      ${Break}
    ${EndIf}
    FindNext $0 $1
  ${Loop}
  FindClose $0
FunctionEnd

Function .onVerifyInstDir
  Call CheckInstallDir
  ${If} $R0 != 1
    Abort
  ${EndIf}
FunctionEnd

; Only SYSTEM and the administrators may change the program; everyone (and Store apps, whose sandbox loads the text
; service) may read and run it.  A folder chosen outside Program Files would otherwise keep C:\'s permissions, which
; let every signed-in user change it, and with it what the update service runs as SYSTEM.
!macro ProtectInstallDir
  nsExec::Exec '"$SYSDIR\icacls.exe" "$INSTDIR" /setowner *S-1-5-32-544 /C /Q'
  Pop $0
  nsExec::Exec '"$SYSDIR\icacls.exe" "$INSTDIR" /inheritance:r /grant:r *S-1-5-18:(OI)(CI)F *S-1-5-32-544:(OI)(CI)F *S-1-5-32-545:(OI)(CI)RX *S-1-15-2-1:(OI)(CI)RX *S-1-15-2-2:(OI)(CI)RX /C /Q'
  Pop $0
  ; What an earlier version installed takes these permissions too (only ZhuyinType's own files: a folder that an
  ; earlier version let be shared keeps everything else as it is).
  !insertmacro ProtectInstalled "${APP}"
  !insertmacro ProtectInstalled "onnxruntime.dll"
  !insertmacro ProtectInstalled "zhuyintype-tsf-*"
  !insertmacro ProtectInstalled "*.ico"
  !insertmacro ProtectInstalled "background.lnk"
  !insertmacro ProtectInstalled "uninstall.exe"
  !insertmacro ProtectInstalled "data"
  !insertmacro ProtectInstalled "licenses"
  !insertmacro ProtectInstalled "fonts"
  !insertmacro ProtectInstalled "updates"
!macroend

!macro ProtectInstalled NAME
  nsExec::Exec '"$SYSDIR\icacls.exe" "$INSTDIR\${NAME}" /setowner *S-1-5-32-544 /T /C /Q'
  Pop $0
  nsExec::Exec '"$SYSDIR\icacls.exe" "$INSTDIR\${NAME}" /reset /T /C /Q'
  Pop $0
!macroend

; The same, in the background (no settings window), through a shortcut: Explorer passes no arguments itself.
Function LaunchInBackgroundAsUser
  Exec '"$WINDIR\explorer.exe" "$INSTDIR\background.lnk"'
FunctionEnd

; The update service: demand start, LocalSystem, `zhuyintype.exe --update-service`.  Its security descriptor lets
; every signed-in user (AU) query and start it and nothing else; administrators and SYSTEM keep full control.  It
; takes no input: it fetches the signed feed itself and installs only a release signed with the project's key.
!macro InstallUpdateService
  nsExec::Exec '"$SYSDIR\sc.exe" create ${UPDATE_SERVICE} binPath= "\"$INSTDIR\${APP}\" --update-service" start= demand obj= LocalSystem DisplayName= "ZhuyinType 更新"'
  Pop $0
  nsExec::Exec '"$SYSDIR\sc.exe" config ${UPDATE_SERVICE} binPath= "\"$INSTDIR\${APP}\" --update-service" start= demand obj= LocalSystem DisplayName= "ZhuyinType 更新"'
  Pop $0
  nsExec::Exec '"$SYSDIR\sc.exe" description ${UPDATE_SERVICE} "安裝 ZhuyinType 經過簽章驗證的新版本"'
  Pop $0
  nsExec::Exec '"$SYSDIR\sc.exe" sdset ${UPDATE_SERVICE} D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)(A;;LCRP;;;AU)'
  Pop $0
!macroend

!macro RemoveUpdateService
  nsExec::Exec '"$SYSDIR\sc.exe" stop ${UPDATE_SERVICE}'
  Pop $0
  nsExec::Exec '"$SYSDIR\sc.exe" delete ${UPDATE_SERVICE}'
  Pop $0
!macroend

; ---------------------------------------------------------------------------------------------------------------
; Install

Function .onInit
  ${IfNot} ${RunningX64}
  ${OrIfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP|MB_OK "ZhuyinType 需要 64 位元的 Windows 10 或 Windows 11。" /SD IDOK
    SetErrorLevel 3
    Abort
  ${EndIf}
  ; ARM Windows runs this installer, but its own programs (Notepad, Explorer, Edge) cannot load an x64 text service.
  ${If} ${IsNativeARM64}
    MessageBox MB_ICONSTOP|MB_OK "ZhuyinType 尚不支援 ARM 版的 Windows。" /SD IDOK
    SetErrorLevel 3
    Abort
  ${EndIf}
  SetShellVarContext all
  StrCpy $IsUpdate 0
  ${GetParameters} $1
  ClearErrors
  ${GetOptions} $1 "/UPDATE" $2
  ${IfNot} ${Errors}
    StrCpy $IsUpdate 1
  ${EndIf}
FunctionEnd

; A failed install (a file in use, the text service not registered) still starts the app again where it ran.
Function .onInstFailed
  ${If} $IsUpdate == 1
    WriteRegStr HKLM "Software\${PRODUCT}" "RelaunchSessions" "$AppSessions"
    Exec '"$INSTDIR\${APP}" --relaunch-sessions'
  ${ElseIf} $AppSessions != ""
    ; The service may still be the earlier version, which takes no start arguments.
    Call LaunchInBackgroundAsUser
  ${EndIf}
FunctionEnd

Section "ZhuyinType" SecMain
  SectionIn RO
  SetShellVarContext all
  Call CheckInstallDir
  ${If} $R0 != 1
    MessageBox MB_ICONSTOP|MB_OK "無法安裝到「$INSTDIR」：請選擇新的或空的資料夾。" /SD IDOK
    SetErrorLevel 3
    Abort
  ${EndIf}
  StrCpy $Upgrade 0
  ${If} ${FileExists} "$INSTDIR\${APP}"
    StrCpy $Upgrade 1
  ${EndIf}
  SetOutPath "$INSTDIR"
  !insertmacro ProtectInstallDir
  Call RecordAppSessions
  !insertmacro StopApp
  !insertmacro DeleteOldCopies
  !insertmacro MoveAside "$INSTDIR\${TSF64}"
  !insertmacro MoveAside "$INSTDIR\${TSF32}"

  File "${STAGE}/${APP}"
  ; ONNX Runtime for the neural models, loaded by the app only (stopped above).
  File "${STAGE}/onnxruntime.dll"
  File "${STAGE}/${TSF64}"
  File "${STAGE}/${TSF32}"
  File "${STAGE}/logo.ico"
  File "${STAGE}/zhuyintype.ico"
  RMDir /r "$INSTDIR\data"
  SetOutPath "$INSTDIR\data"
  File /r "${STAGE}/data/*"
  RMDir /r "$INSTDIR\licenses"
  SetOutPath "$INSTDIR\licenses"
  File /r "${STAGE}/licenses/*"
  ; The candidate window's typeface, read by the text service inside running applications: a file in use stays as
  ; it is (the same font) rather than failing the upgrade.
  SetOutPath "$INSTDIR\fonts"
  SetOverwrite try
  File /r "${STAGE}/fonts/*"
  SetOverwrite on
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; The text service: 64-bit for 64-bit applications, 32-bit (WOW64 registry view) for 32-bit ones.
  ExecWait '"$SYSDIR\regsvr32.exe" /s "$INSTDIR\${TSF64}"' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP|MB_OK "無法登記 ZhuyinType 輸入法（64 位元，錯誤 $0）。" /SD IDOK
    SetErrorLevel 2
    Abort
  ${EndIf}
  ExecWait '"$WINDIR\SysWOW64\regsvr32.exe" /s "$INSTDIR\${TSF32}"' $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "無法登記 32 位元程式用的 ZhuyinType 輸入法（錯誤 $0）。64 位元程式仍可使用。" /SD IDOK
  ${EndIf}
  ; The installing user's input method list (Win+Space); other users get it from Active Setup at sign-in.
  ExecWait '"$SYSDIR\regsvr32.exe" /s /n /i:user "$INSTDIR\${TSF64}"' $0
  WriteRegStr HKLM "${ACTIVE_SETUP_KEY}" "" "ZhuyinType 注音"
  WriteRegExpandStr HKLM "${ACTIVE_SETUP_KEY}" "StubPath" '"%SystemRoot%\System32\regsvr32.exe" /s /n /i:user "$INSTDIR\${TSF64}"'
  WriteRegStr HKLM "${ACTIVE_SETUP_KEY}" "Version" "1"
  WriteRegDWORD HKLM "${ACTIVE_SETUP_KEY}" "IsInstalled" 1
  WriteRegStr HKCU "${ACTIVE_SETUP_KEY}" "Version" "1"

  ; Every user who signs in gets the input method host (the profile is registered for the whole machine).
  WriteRegStr HKLM "${RUN_KEY}" "ZhuyinType" '"$INSTDIR\${APP}" --background'
  CreateShortcut "$INSTDIR\background.lnk" "$INSTDIR\${APP}" "--background" "$INSTDIR\zhuyintype.ico" 0
  CreateShortcut "$SMPROGRAMS\${SHORTCUT}" "$INSTDIR\${APP}" "" "$INSTDIR\zhuyintype.ico" 0 SW_SHOWNORMAL "" "開啟 ZhuyinType 設定"

  ; Again for the files just written: their owner is whoever ran the installer, which a policy may make the
  ; administrator's own account (the update service accepts only SYSTEM, Administrators or TrustedInstaller).
  !insertmacro ProtectInstallDir
  !insertmacro InstallUpdateService
  ; The update service's download of this installer (when it runs this, at the next restart).
  RMDir /r /REBOOTOK "$INSTDIR\updates"
  SetRebootFlag false

  WriteRegStr HKLM "Software\${PRODUCT}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\${PRODUCT}" "Version" "${VERSION}"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayName" "ZhuyinType 注音輸入法"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\zhuyintype.ico"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "URLInfoAbout" "${WEBSITE}"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${UNINSTALL_KEY}" "EstimatedSize" "$0"

  ; The app starts again in the background, as each user, wherever it ran before (in every session, for whoever
  ; installed or updated).  A first silent install starts it for everyone signed in, so it can be used at once.  The
  ; finish page offers to open its settings.
  ${If} $AppSessions == ""
  ${AndIf} $Upgrade == 0
  ${AndIf} ${Silent}
    StrCpy $AppSessions "all"
  ${EndIf}
  Call RelaunchApp
SectionEnd

; ---------------------------------------------------------------------------------------------------------------
; Uninstall

Function un.onInit
  SetShellVarContext all
FunctionEnd

Section "un.ZhuyinType" UnSecMain
  SectionIn RO
  SetShellVarContext all
  !insertmacro RemoveUpdateService
  !insertmacro StopApp
  ; The uninstalling user's list first, while the profile is still registered.
  ExecWait '"$SYSDIR\regsvr32.exe" /s /u /n /i:user "$INSTDIR\${TSF64}"'
  ExecWait '"$WINDIR\SysWOW64\regsvr32.exe" /s /u "$INSTDIR\${TSF32}"'
  ExecWait '"$SYSDIR\regsvr32.exe" /s /u "$INSTDIR\${TSF64}"'

  DeleteRegKey HKLM "${ACTIVE_SETUP_KEY}"
  DeleteRegKey HKCU "${ACTIVE_SETUP_KEY}"
  DeleteRegValue HKLM "${RUN_KEY}" "ZhuyinType"
  Delete "$SMPROGRAMS\${SHORTCUT}"

  ; The DLLs are still loaded in applications that used the input method: renamed, deleted when they exit or at
  ; the next restart.
  !insertmacro MoveAside "$INSTDIR\${TSF64}"
  !insertmacro MoveAside "$INSTDIR\${TSF32}"
  !insertmacro DeleteOldCopies
  Delete "$INSTDIR\${APP}"
  Delete "$INSTDIR\onnxruntime.dll"
  Delete "$INSTDIR\logo.ico"
  Delete "$INSTDIR\zhuyintype.ico"
  Delete "$INSTDIR\background.lnk"
  RMDir /r "$INSTDIR\data"
  RMDir /r "$INSTDIR\licenses"
  RMDir /r "$INSTDIR\updates"
  RMDir /r /REBOOTOK "$INSTDIR\fonts"
  Delete "$INSTDIR\uninstall.exe"
  RMDir /REBOOTOK "$INSTDIR"
  SetRebootFlag false

  DeleteRegKey HKLM "${UNINSTALL_KEY}"
  DeleteRegKey HKLM "Software\${PRODUCT}"
SectionEnd

; Off by default: personal data stays for a later reinstall.
Section /o "un.同時刪除我的自訂詞、學習記錄與設定" UnSecData
  SetShellVarContext current
  RMDir /r "$APPDATA\${PRODUCT}"
  RMDir /r "$LOCALAPPDATA\${PRODUCT}"
  SetShellVarContext all
SectionEnd

!insertmacro MUI_UNFUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${UnSecMain} "程式、輸入法登記、登入時啟動與開始功能表捷徑。"
  !insertmacro MUI_DESCRIPTION_TEXT ${UnSecData} "目前使用者的自訂詞、學習記錄、使用統計、改字記錄與設定（%APPDATA% 與 %LOCALAPPDATA% 的 ZhuyinType 資料夾）。"
!insertmacro MUI_UNFUNCTION_DESCRIPTION_END
