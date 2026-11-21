# Use FlipClock as a screen saver

English · [简体中文](../zh-CN/screensaver.md)

FlipClock is a real Windows screen saver, not a video or a lock-screen replacement.
`FlipClock.scr` is the same binary as the app; Windows starts it with `/s` when the
screen saver fires. Registration is per user.

## Install

1. Open **Settings → Saver**.
2. Click **Install Screen Saver**.
3. Set the wait time in Windows: **Saver → Open Windows Screen Saver Settings**.

What the install does:

- Copies the running `FlipClock.exe` to `%LOCALAPPDATA%\FlipClock\FlipClock.scr`.
- Writes two values under `HKCU\Control Panel\Desktop`: `SCRNSAVE.EXE` (the path above)
  and `ScreenSaveActive` (`1`).
- Records your previous screen saver as a rollback point.

No administrator rights are needed. FlipClock does not write to `System32` and does not
replace the Windows lock screen.

## Why it disappears from the Windows list

The Windows screen saver list shows the files in `System32\*.scr` **plus the currently
selected saver**. FlipClock gets into that list as the current value, not because it lives
in `System32`. Pick another saver and FlipClock drops out — click **Install Screen Saver**
again to bring it back.

Making it a permanent list entry would mean writing to `System32`, which needs
administrator rights; the project deliberately does not do that.

## Test and exit

- The small **Preview** in the Windows dialog stays blank. FlipClock does not implement the
  `/p <HWND>` system-preview hook, by design.
- Use **Preview Screen Saver** in the app, or run `FlipClock.exe /s`.
- Any key, any mouse button, or pointer movement over 3 px ends the session. The first
  700 ms after launch are ignored so a jittery start does not close it.

## Multiple monitors

**Saver → Multiple monitors** offers **Primary monitor** and **All monitors**. A specific
subset of monitors is not supported. If a monitor is added, removed, or its resolution
changes, the running session rebuilds itself.

## Restore, reinstall, update needed

- **Restore Original Screen Saver** returns the selection recorded before install.
- Uninstalling FlipClock restores it automatically.
- At startup the app compares the registered `.scr` with itself and refreshes the copy when
  they differ. If the copy cannot be replaced (the saver is running, or the file is locked),
  the preview-column status strip says the saver is outdated and the Saver page shows
  **Update Needed**. Close the saver, then click **Update Screen Saver**.

## Command line

| Argument | Effect |
| --- | --- |
| *(none)* | Settings window or full-screen clock, per **No-argument launch** |
| `/c` | Open the Settings window |
| `/s` | Start the screen saver; any input exits |
| `/p <HWND>` | Not supported — the process exits without creating a window |
| `--fullscreen` | Full-screen clock on the primary monitor, always on top |
| `--tray` | Tray only, no window; used by autostart |
| `--restore-screensaver` | Used by the uninstaller to restore your previous saver |
| `--debug-windowed` | Development only: a 960×540 saver window instead of full screen |

`/s` and `/p` bypass the single-instance check so Windows can always start the saver.
