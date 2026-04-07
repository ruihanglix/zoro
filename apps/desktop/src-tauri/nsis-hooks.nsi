; Zoro NSIS installer hooks — Add/remove zoro CLI from user PATH
; Referenced by tauri.conf.json: bundle.windows.nsis.installerHooks
;
; Tauri v2 NSIS hook macros:
;   NSIS_HOOK_PREINSTALL    — before files are installed
;   NSIS_HOOK_POSTINSTALL   — after files are installed
;   NSIS_HOOK_PREUNINSTALL  — before uninstall starts
;   NSIS_HOOK_POSTUNINSTALL — after uninstall completes

!macro NSIS_HOOK_POSTINSTALL
  ; Copy the zoro sidecar binary to a stable location on PATH
  CreateDirectory "$LOCALAPPDATA\Zoro\bin"
  CopyFiles /SILENT "$INSTDIR\zoro.exe" "$LOCALAPPDATA\Zoro\bin\zoro.exe"

  ; Add to user PATH via registry if not already present
  ReadRegStr $0 HKCU "Environment" "Path"
  StrCmp $0 "" 0 +3
    ; PATH is empty — set it
    WriteRegExpandStr HKCU "Environment" "Path" "$LOCALAPPDATA\Zoro\bin"
    Goto path_done
  ; PATH exists — check if already contains our dir
  StrCpy $1 "$0"
  Push "$1"
  Push "$LOCALAPPDATA\Zoro\bin"
  Call StrContains
  Pop $2
  StrCmp $2 "" 0 path_done
    ; Not found — append
    WriteRegExpandStr HKCU "Environment" "Path" "$0;$LOCALAPPDATA\Zoro\bin"
  path_done:

  ; Broadcast WM_SETTINGCHANGE so new terminals pick up the PATH change
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Remove the CLI binary
  Delete "$LOCALAPPDATA\Zoro\bin\zoro.exe"
  RMDir "$LOCALAPPDATA\Zoro\bin"
  RMDir "$LOCALAPPDATA\Zoro"

  ; Remove our entry from user PATH
  ReadRegStr $0 HKCU "Environment" "Path"
  StrCmp $0 "" path_cleanup_done
    ; Try removing ";$LOCALAPPDATA\Zoro\bin" (middle/end of PATH)
    ${WordReplace} $0 ";$LOCALAPPDATA\Zoro\bin" "" "+" $1
    ; Try removing "$LOCALAPPDATA\Zoro\bin;" (start of PATH)
    ${WordReplace} $1 "$LOCALAPPDATA\Zoro\bin;" "" "+" $2
    ; Try removing "$LOCALAPPDATA\Zoro\bin" (only entry in PATH)
    ${WordReplace} $2 "$LOCALAPPDATA\Zoro\bin" "" "+" $3
    StrCmp $3 $0 path_cleanup_done
      WriteRegExpandStr HKCU "Environment" "Path" "$3"
      SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
  path_cleanup_done:
!macroend

; Helper function: check if string $1 contains substring $2
; Returns result in stack (empty = not found)
Function StrContains
  Exch $R1 ; substring
  Exch
  Exch $R0 ; string
  Push $R2
  Push $R3
  Push $R4
  StrLen $R3 $R1
  StrCpy $R4 0
  loop:
    StrCpy $R2 $R0 $R3 $R4
    StrCmp $R2 "" notfound
    StrCmp $R2 $R1 found
    IntOp $R4 $R4 + 1
    Goto loop
  found:
    StrCpy $R0 $R1
    Goto done
  notfound:
    StrCpy $R0 ""
  done:
  Pop $R4
  Pop $R3
  Pop $R2
  Exch $R0
  Exch
  Pop $R1
FunctionEnd
