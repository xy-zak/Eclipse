; Eclipse NSIS hooks. Firewall + SunshineService (boot-time host).
; The running Eclipse GUI does not require Administrator.

; Upgrades: the running host locks tools\sunshinesvc.exe and the sidecars, so
; stop it before files are copied. POSTINSTALL recreates and restarts it.
!macro NSIS_HOOK_PREINSTALL
  nsExec::ExecToLog 'net stop SunshineService'
  Pop $0
  nsExec::ExecToLog 'taskkill /F /T /IM sunshinesvc.exe'
  Pop $0
  nsExec::ExecToLog 'powershell.exe -NoProfile -Command "Get-Process sunshine,moonlight -ErrorAction SilentlyContinue | Where-Object { $$_.Path -like \"$INSTDIR\*\" } | Stop-Process -Force"'
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Eclipse GameStream TCP (in)"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="Eclipse GameStream TCP (in)" dir=in action=allow protocol=TCP localport=47984-47990,48010 remoteip=100.64.0.0/10 profile=any'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Eclipse GameStream UDP (in)"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="Eclipse GameStream UDP (in)" dir=in action=allow protocol=UDP localport=47998-48010 remoteip=100.64.0.0/10 profile=any'
  Pop $0

; Recreate so upgrades pick up a new sunshinesvc.exe. SCM name must stay
; SunshineService (hardcoded in vendor/sunshine/tools/sunshinesvc.cpp).
; Do not install official Sunshine alongside Eclipse. sunshinesvc blocks
; idle sleep while Sunshine.exe is running; lid-close still sleeps.
  nsExec::ExecToLog 'net stop SunshineService'
  Pop $0
  nsExec::ExecToLog 'sc delete SunshineService'
  Pop $0
  StrCpy $0 '"$INSTDIR\tools\sunshinesvc.exe"'
  nsExec::ExecToLog 'sc create SunshineService binPath= $0 start= delayed-auto DisplayName= "Eclipse Host"'
  Pop $0
  nsExec::ExecToLog 'sc description SunshineService "Eclipse Sunshine host. Do not install official Sunshine alongside Eclipse."'
  Pop $0
  nsExec::ExecToLog 'sc failure SunshineService reset= 86400 actions= restart/5000/restart/5000/restart/5000'
  Pop $0
  SetOutPath "$INSTDIR"
  nsExec::ExecToLog 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\seed-host.ps1" -InstallDir "$INSTDIR"'
  Pop $0
  nsExec::ExecToLog 'net start SunshineService'
  Pop $0
  nsExec::ExecToLog '"$INSTDIR\grant-host-acl.cmd"'
  Pop $0
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "Eclipse" '"$INSTDIR\eclipse.exe" --tray'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'net stop SunshineService'
  Pop $0
  nsExec::ExecToLog 'sc delete SunshineService'
  Pop $0
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "Eclipse"
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Eclipse GameStream TCP (in)"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Eclipse GameStream UDP (in)"'
  Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
