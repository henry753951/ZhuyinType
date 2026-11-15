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
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchAsUser
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

Var AppWasRunning

; Whether the app runs in this session, so an upgrade started there (silently, by an update) can start it again.
Function RecordAppRunning
  StrCpy $AppWasRunning 0
  System::Call 'kernel32::GetCurrentProcessId() i .r0'
  System::Call 'kernel32::ProcessIdToSessionId(i r0, *i .r1)'
  nsExec::ExecToStack '"$SYSDIR\tasklist.exe" /NH /FI "IMAGENAME eq ${APP}" /FI "SESSION eq $1"'
  Pop $0
  Pop $2
  ${StrStr} $3 $2 "${APP}"
  ${If} $3 != ""
    StrCpy $AppWasRunning 1
  ${EndIf}
FunctionEnd

; Ends the running app (the input method host) for every user: first politely, then for good.
!macro StopApp
  nsExec::Exec '"$SYSDIR\taskkill.exe" /IM ${APP}'
  Pop $0
  Sleep 1500
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM ${APP}'
  Pop $0
  Sleep 300
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

; The same, in the background (no settings window), through a shortcut: Explorer passes no arguments itself.
Function LaunchInBackgroundAsUser
  Exec '"$WINDIR\explorer.exe" "$INSTDIR\background.lnk"'
FunctionEnd

; ---------------------------------------------------------------------------------------------------------------
; Install

Function .onInit
  ${IfNot} ${RunningX64}
  ${OrIfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP|MB_OK "ZhuyinType 需要 64 位元的 Windows 10 或 Windows 11。"
    Abort
  ${EndIf}
  SetShellVarContext all
FunctionEnd

Section "ZhuyinType" SecMain
  SectionIn RO
  SetShellVarContext all
  SetOutPath "$INSTDIR"
  Call RecordAppRunning
  !insertmacro StopApp
  !insertmacro DeleteOldCopies
  !insertmacro MoveAside "$INSTDIR\${TSF64}"
  !insertmacro MoveAside "$INSTDIR\${TSF32}"

  File "${STAGE}/${APP}"
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
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; The text service: 64-bit for 64-bit applications, 32-bit (WOW64 registry view) for 32-bit ones.
  ExecWait '"$SYSDIR\regsvr32.exe" /s "$INSTDIR\${TSF64}"' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP|MB_OK "無法登記 ZhuyinType 輸入法（64 位元，錯誤 $0）。"
    SetErrorLevel 2
    Abort
  ${EndIf}
  ExecWait '"$WINDIR\SysWOW64\regsvr32.exe" /s "$INSTDIR\${TSF32}"' $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "無法登記 32 位元程式用的 ZhuyinType 輸入法（錯誤 $0）。64 位元程式仍可使用。"
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

  ; The finish page offers to start the app.  A silent install starts it again only if it was running in this
  ; session (an upgrade); otherwise it starts at the next sign-in.
  ${If} ${Silent}
  ${AndIf} $AppWasRunning == 1
    Call LaunchInBackgroundAsUser
  ${EndIf}
SectionEnd

; ---------------------------------------------------------------------------------------------------------------
; Uninstall

Function un.onInit
  SetShellVarContext all
FunctionEnd

Section "un.ZhuyinType" UnSecMain
  SectionIn RO
  SetShellVarContext all
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
  Delete "$INSTDIR\logo.ico"
  Delete "$INSTDIR\zhuyintype.ico"
  Delete "$INSTDIR\background.lnk"
  RMDir /r "$INSTDIR\data"
  RMDir /r "$INSTDIR\licenses"
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
