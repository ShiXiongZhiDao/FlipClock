# Troubleshooting

English · [简体中文](../zh-CN/troubleshooting.md)

Start with the exact message or the page where it appears. If a cause is not confirmed by
the code, it is phrased as something to check rather than a fact.

## The app will not install or start

- **SmartScreen on first run** — the installer has no code-signing certificate. Choose
  **More info → Run anyway**. The update packages are signed separately.
- **WebView2 missing** — rare on Windows 10/11. Install the Microsoft Edge WebView2 Runtime,
  then start FlipClock again.
- **Portable build does not start** — unzip the whole archive first; running `FlipClock.exe`
  from inside the zip does not work.

## The screen saver does not appear

- **Not installed** — Settings → Saver → **Install Screen Saver**.
- **It vanished from the Windows list** — the list shows `System32\*.scr` plus the current
  selection. Choosing another saver removes FlipClock; install it again to bring it back.
- **Windows Preview is blank** — `/p` system preview is not supported by design. Use
  **Preview Screen Saver** in the app.
- **Update Needed** — close any running saver, then click
  **Update Screen Saver**.
- **The wait time is ignored** — it is a Windows setting, not a FlipClock setting. Open it
  from the Saver page.

## The tray icon stays after closing the window

That is the default: closing destroys the window; the app stays in the tray. Right-click the icon → **Quit**
to exit, or turn off **Keep the app in the system tray when closing the window** in
System → Startup & Window. In 1.1.0 this switch did not survive a restart; 1.1.1 fixes it.

## Updates fail

- The installed build needs access to Gitee or GitHub. Gitee is tried first; GitHub is used
  only when Gitee returns an error, not when it times out.
- The portable build can check for updates but cannot install them. Download the new zip.

## Fonts look wrong

- Accepted formats are `ttf`, `otf`, `woff`, `woff2`, up to 32 MB each and 32 fonts total.
- Deleting a font makes themes that used it fall back to **Bebas Neue**.
- Some fonts have uneven digit heights. The app warns about it but still accepts the font;
  choose another if the clock looks crooked.

## Images do not show

- Imported images are copied into the configuration directory. Moving the originals is safe;
  deleting them inside the app is not reversible.
- Supported formats: `png`, `jpg`, `jpeg`, `webp`.
- A low-contrast warning means digits and cards are too close in brightness; adjust one of them.

## Multiple monitors

**Primary monitor** and **All monitors** are supported. A subset is not. When a monitor is
added or removed, a running saver rebuilds its windows.

## Config import fails

- Only bundles with format 1 and schema 4 through 5 are accepted (FlipClock 1.1.2
  exports schema 5).
- Import merges custom themes and fonts; it never deletes what is already on the machine.
  Name conflicts become copies.
- Check `backups\pre-import.zip` in the configuration directory if you need to go back.
