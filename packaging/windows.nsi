Unicode True
!include "MUI2.nsh"
!include "x64.nsh"

Name "openpaint"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\openpaint"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "openpaint"
VIAddVersionKey "FileDescription" "openpaint installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "openpaint contributors"
!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_LICENSE "${PAYLOAD}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "openpaint requires 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Section "openpaint"
  SetOutPath "$INSTDIR"
  File "${PAYLOAD}\openpaint.exe"
  File "${PAYLOAD}\LICENSE"
  File "${PAYLOAD}\THIRD_PARTY.md"
  File "${PAYLOAD}\DejaVuSans-LICENSE.txt"
  File "${PAYLOAD}\ProggyClean-LICENSE.txt"
  File "${PAYLOAD}\openpaint.ico"
  SetOutPath "$INSTDIR\licenses"
  File /r "${PAYLOAD}\licenses\*"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateDirectory "$SMPROGRAMS\openpaint"
  CreateShortcut "$SMPROGRAMS\openpaint\openpaint.lnk" "$INSTDIR\openpaint.exe" "" "$INSTDIR\openpaint.ico"
  CreateShortcut "$SMPROGRAMS\openpaint\uninstall.lnk" "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\openpaint" "DisplayName" "openpaint"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\openpaint" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\openpaint" "DisplayIcon" "$INSTDIR\openpaint.ico"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\openpaint" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\openpaint" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\openpaint" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\openpaint" "NoRepair" 1
  WriteRegStr HKCU "Software\Classes\openpaint.Image" "" "openpaint layered image"
  WriteRegStr HKCU "Software\Classes\openpaint.Image\DefaultIcon" "" "$INSTDIR\openpaint.ico"
  WriteRegStr HKCU "Software\Classes\openpaint.Image\shell\open\command" "" '$\"$INSTDIR\openpaint.exe$\" $\"%1$\"'
  WriteRegStr HKCU "Software\Classes\.opaint\OpenWithProgids" "openpaint.Image" ""
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "Uninstall"
  SetRegView 64
  Delete "$SMPROGRAMS\openpaint\openpaint.lnk"
  Delete "$SMPROGRAMS\openpaint\uninstall.lnk"
  RMDir "$SMPROGRAMS\openpaint"
  Delete "$INSTDIR\openpaint.exe"
  Delete "$INSTDIR\openpaint.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\THIRD_PARTY.md"
  Delete "$INSTDIR\DejaVuSans-LICENSE.txt"
  Delete "$INSTDIR\ProggyClean-LICENSE.txt"
  RMDir /r "$INSTDIR\licenses"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\openpaint"
  DeleteRegKey HKCU "Software\Classes\openpaint.Image"
  DeleteRegValue HKCU "Software\Classes\.opaint\OpenWithProgids" "openpaint.Image"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd
