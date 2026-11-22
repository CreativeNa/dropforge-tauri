; Хуки установщика DropForge.
; Перед удалением приложения стираем профиль с токеном DPAPI и кэшем,
; чтобы после деинсталляции не оставалось данных аккаунта.

!macro NSIS_HOOK_PREUNINSTALL
  RMDir /r "$INSTDIR\data"
!macroend
