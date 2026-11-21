# Privacy

English · [简体中文](../zh-CN/privacy.md)

FlipClock does not collect personal data. There is no account, no name or email address, no
device or installation ID, and no access to your documents, clipboard, camera, or
microphone.

## Anonymous usage analytics

Official builds send five anonymous events: app start, a snapshot of the clock configuration,
Settings opened, a feature toggled, and an update check. No free text is sent. A temporary
session ID groups events from one run; it lives in memory, is not written to disk, and is
not a user identifier. Events go to Aptabase (`eu.aptabase.com`), a third-party analytics
service. Builds compiled from source without the app key send nothing. There is no in-app
switch.

## Other network requests

- **Update check** — about five seconds after the Settings window opens, and when you click
  **Check for updates**. Gitee is tried first, GitHub is the fallback.
- **About images** — three images load from GitHub or Gitee when you open the System tab.

There is no account, no sync, and no cloud storage.

## Local data

Settings, themes, fonts, and imported images live in `%APPDATA%\com.dd.shixiongzhidao\`.
Imported files are copied there; moving the originals does not affect the app. FlipClock
writes no logs or crash reports.

## Third-party infrastructure

The services above may process connection metadata such as IP addresses under their own
policies. This project does not control their retention.
