!macro NSIS_HOOK_POSTINSTALL
  ; Register the custom URL scheme used by the Photoshop UXP plugin.
  WriteRegStr HKCR "comfypanel-turbo-bar" "" "URL:ComfyPanel Turbo Bar Protocol"
  WriteRegStr HKCR "comfypanel-turbo-bar" "URL Protocol" ""
  WriteRegStr HKCR "comfypanel-turbo-bar\shell\open\command" "" '"$INSTDIR\comfypanel-turbo-bar.exe" "%1"'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Remove the custom URL scheme when Turbo Bar is uninstalled.
  DeleteRegKey HKCR "comfypanel-turbo-bar"
!macroend