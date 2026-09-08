# klenua build

klenua build is a local desktop utility for safely viewing and updating app versions and build numbers. It supports Flutter, native Android Gradle projects, native iOS/macOS Xcode projects, Electron, and Tauri.

## What it does

- Detects `pubspec.yaml`, `build.gradle` / `build.gradle.kts`, and Xcode `project.pbxproj` files.
- Detects Electron `package.json` and Tauri `tauri.conf.json` files, updating only static version fields and Electron `build.buildNumber` values.
- Uses Flutter's `pubspec.yaml` as the source of truth, leaving Flutter-derived native values alone.
- Creates a targeted, diff-style preview before every write.
- Refuses dynamic or ambiguous Gradle/Xcode/Info.plist values and asks for manual review.
- Saves the previous contents of every changed file under `.klenuabuild-backup/` and can restore them.
- Stores local recent projects in the operating system app-data directory.
- Lets you remove an entry from Saved projects without deleting its project folder or files.
- Optionally writes `.versionpilot.json` after a successful apply when “Remember configuration” remains enabled.

## Requirements

- Node.js 18+
- Rust stable and Cargo
- Tauri platform prerequisites: [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/)

## Run in development

```bash
npm install
npm run tauri dev
```

## Build a desktop bundle

```bash
npm run tauri build
```

## Safety model

Kleuna Build reads source files, identifies known structured fields, builds replacements in memory, and displays them before writing. It does not use global version-string replacement. On Apply, it writes a per-file backup to `.klenuabuild-backup/` first. The backup mirrors the modified project-relative paths and is overwritten on the next apply, so it represents the latest Kleuna Build change.

For Xcode, `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` are updated only when they are static values. `Info.plist` values containing `$(...)` are deliberately not modified. For Android, direct static `versionName` / `versionCode` declarations are supported; dynamically calculated declarations are rejected.

## Keyboard shortcuts

- `Cmd/Ctrl + O` — open project
- `Cmd/Ctrl + Enter` — preview/apply changes
- `Cmd/Ctrl + Shift + B` — increment build
