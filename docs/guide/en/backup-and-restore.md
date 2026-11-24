# Back up and move your setup

English · [简体中文](../zh-CN/backup-and-restore.md)

A config bundle carries your themes, fonts, and preferences to another PC. It is not a
cloud backup: the file stays wherever you save it.

## Export

**System → Config migration → Export config bundle…**

The file is named `FlipClock-config-YYYYMMDD.zip`. Export is always complete; there is
nothing to tick on the way out.

It contains:

- All custom themes, including their background and texture images
- The whole imported-font library
- Every theme's clock profile (format, date, seconds, separator dots, layout, font, scale,
  brightness, sound)
- The current theme
- Preferences: interface language, UI appearance, monitor mode, no-argument launch,
  keeping the app in the tray after closing

It does **not** contain the screen saver registration or rollback point, and it does not
contain autostart — those live in the registry.

## Import

**System → Config migration → Import config bundle…**, choose the zip, then tick what to
restore:

| Group | Behaviour |
| --- | --- |
| Built-in theme clock profiles | Overwrites the matching profiles |
| Custom themes | Merged; identical themes map to the existing one, name or id conflicts become a copy |
| Fonts | Merged by content; duplicates are reused |
| Preferences | Overwrites the four preference sections |

If the current theme is part of the selection, it becomes current after import.

Before writing anything, the app saves the existing setup to
`%APPDATA%\com.dd.shixiongzhidao\backups\pre-import.zip`. If that backup fails, the import
stops.

## Version compatibility

Bundles with format 1 and schema 4 through 5 are accepted (FlipClock 1.1.2 exports
schema 5). Older or newer bundles are rejected with a message; there is no downgrade or
migration path.

## Limits

Up to 36 custom themes and 32 imported fonts. Items beyond the limit are skipped in import
order and reported when the import finishes.
