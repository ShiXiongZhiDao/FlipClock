; NSIS 安装/卸载钩子（tauri.conf.json → bundle.windows.nsis.installerHooks）。
;
; 安装：什么都不做。不动用户的屏保选择——「组件安装」是应用内的一次显式动作，
; 装完应用顺手改掉别人的屏保属于越权（ADR-0014）。
;
; 卸载：先把屏保选择还回去。应用里的 install::restore 会读回滚点，
; 把 SCRNSAVE.EXE 恢复成用户原来的值，再清掉自己的登记。
; 必须在文件被删之前跑，否则 exe 已经不在了。
; 参数走 --restore-screensaver：这个模式绕过单实例、不开窗口、跑完即退。

!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "$INSTDIR\FlipClock.exe" 0 +2
    ExecWait '"$INSTDIR\FlipClock.exe" --restore-screensaver'
!macroend
