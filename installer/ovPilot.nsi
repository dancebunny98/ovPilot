!include "MUI2.nsh"

!ifndef SOURCE_EXE
  !define SOURCE_EXE "..\target\release\ovPilot.exe"
!endif
!ifndef PRODUCT_VERSION
  !define PRODUCT_VERSION "1.0.0"
!endif

Name "ovPilot ${PRODUCT_VERSION}"
OutFile "ovPilot-Setup-${PRODUCT_VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\ovPilot"
RequestExecutionLevel user
Unicode true

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "ovPilot" Main
  SetOutPath "$INSTDIR"
  File "${SOURCE_EXE}"
  File /oname=AircraftTypes-LICENSE.txt "..\data\aircraft_types-LICENSE"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\ovPilot"
  CreateShortcut "$SMPROGRAMS\ovPilot\ovPilot.lnk" "$INSTDIR\ovPilot.exe"
  CreateShortcut "$SMPROGRAMS\ovPilot\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ovPilot" "DisplayName" "ovPilot ${PRODUCT_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ovPilot" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ovPilot" "UninstallString" '"$INSTDIR\Uninstall.exe"'
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\ovPilot.exe"
  Delete "$INSTDIR\AircraftTypes-LICENSE.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\ovPilot\ovPilot.lnk"
  Delete "$SMPROGRAMS\ovPilot\Uninstall.lnk"
  RMDir "$SMPROGRAMS\ovPilot"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ovPilot"
SectionEnd
