use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

const IGNORED: &[&str] = &[
    "node_modules",
    "vendor",
    ".git",
    "build",
    "dist",
    "DerivedData",
    "Pods",
    ".dart_tool",
    "generated",
    ".klenuabuild-backup",
    ".versionpilot-backup",
];

/// File paths a project *might* have, found by walking `root` once. Existence
/// of `pubspec`/`electron_package_candidate` is deliberately left for the
/// caller to check (`.is_file()`) rather than filtered here, so detection
/// priority (Flutter, then Tauri, then Electron, ...) stays exactly the
/// short-circuiting sequence each command already implements — this struct
/// only avoids re-walking the filesystem for each one.
pub struct DetectedFiles {
    pub pubspec: PathBuf,
    pub pbx: Option<PathBuf>,
    pub info_plists: Vec<PathBuf>,
    pub gradle: Option<PathBuf>,
    pub tauri_config: Option<PathBuf>,
    pub electron_package_candidate: PathBuf,
}

pub fn detect(root: &Path) -> Result<DetectedFiles, String> {
    if !root.is_dir() {
        return Err("Please choose a valid project folder.".into());
    }
    let files = walk(root)?;
    let pbx = files
        .iter()
        .find(|p| p.to_string_lossy().ends_with(".xcodeproj/project.pbxproj"))
        .cloned();
    let info_plists: Vec<PathBuf> = files
        .iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some("Info.plist"))
        .cloned()
        .collect();
    let gradle = [
        root.join("android/app/build.gradle"),
        root.join("android/app/build.gradle.kts"),
        root.join("app/build.gradle"),
        root.join("app/build.gradle.kts"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    let tauri_config = [
        root.join("src-tauri/tauri.conf.json"),
        root.join("tauri.conf.json"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    Ok(DetectedFiles {
        pubspec: root.join("pubspec.yaml"),
        pbx,
        info_plists,
        gradle,
        tauri_config,
        electron_package_candidate: root.join("package.json"),
    })
}

pub fn is_electron_project(file: &Path) -> Result<bool, String> {
    let json: Value = serde_json::from_str(&read(file)?)
        .map_err(|error| format!("Could not parse {}: {error}", file.display()))?;
    let has_electron = |key: &str| {
        json.get(key)
            .and_then(Value::as_object)
            .map(|items| items.contains_key("electron"))
            .unwrap_or(false)
    };
    Ok(has_electron("dependencies") || has_electron("devDependencies"))
}

pub fn name(root: &Path) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Project")
        .to_string()
}

pub fn rel(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .into()
}

pub fn read(file: &Path) -> Result<String, String> {
    fs::read_to_string(file).map_err(|e| format!("Could not read {}: {}", file.display(), e))
}

pub(crate) fn walk(root: &Path) -> Result<Vec<PathBuf>, String> {
    fn go(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let p = e.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                if !IGNORED
                    .iter()
                    .any(|n| p.file_name().and_then(|x| x.to_str()) == Some(*n))
                {
                    go(&p, out)?
                }
            } else {
                out.push(p)
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    go(root, &mut out)?;
    Ok(out)
}
