use crate::core::project::{self, is_electron_project, name, read, rel};
use crate::git::{self, GitInfo};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionTarget {
    pub id: String,
    pub platform: String,
    pub file: String,
    pub version: Option<String>,
    pub build: Option<String>,
    pub editable: bool,
    pub source_of_truth: bool,
    pub note: Option<String>,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScan {
    pub root: String,
    pub project_name: String,
    pub project_type: String,
    pub platforms: Vec<String>,
    pub version: Option<String>,
    pub build: Option<String>,
    pub source_of_truth: Option<String>,
    pub targets: Vec<VersionTarget>,
    pub warnings: Vec<String>,
    pub git: Option<GitInfo>,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct RecentProject {
    pub path: String,
    pub name: String,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub save_projects: bool,
    pub auto_update: bool,
    #[serde(default = "default_auto_sync_after_apply")]
    pub auto_sync_after_apply: bool,
    #[serde(default)]
    pub upload_backup_files: bool,
}

fn default_auto_sync_after_apply() -> bool { true }
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            save_projects: true,
            auto_update: true,
            auto_sync_after_apply: true,
            upload_backup_files: false,
        }
    }
}

pub fn scan(root: &Path) -> Result<ProjectScan, String> {
    let detected = project::detect(root)?;
    let identity = crate::rename::current_identity(root)?;
    let pubspec = detected.pubspec;
    let pbx = detected.pbx;
    let info_plists = detected.info_plists;
    let gradle = detected.gradle;
    let flutter = pubspec.is_file();
    let tauri_config = detected.tauri_config;
    let electron_package = detected.electron_package_candidate;
    let mut targets = Vec::new();
    let mut warnings = Vec::new();
    let mut platforms = Vec::new();
    if flutter {
        let (version, build) = parse_flutter(&pubspec)?;
        targets.push(target(
            root, "flutter", "Flutter", &pubspec, version, build, true, true, None,
        ));
        if pbx.is_some() {
            platforms.push("iOS".into());
            targets.push(target(
                root,
                "ios",
                "iOS",
                pbx.as_ref().unwrap(),
                None,
                None,
                false,
                false,
                Some("Controlled by pubspec.yaml".into()),
            ));
        }
        if gradle.is_some() {
            platforms.push("Android".into());
            targets.push(target(
                root,
                "android",
                "Android",
                gradle.as_ref().unwrap(),
                None,
                None,
                false,
                false,
                Some("Controlled by pubspec.yaml".into()),
            ));
        }
        let main = targets.first().unwrap();
        return Ok(ProjectScan {
            root: root.to_string_lossy().into(),
            project_name: name(root),
            project_type: "Flutter".into(),
            platforms: {
                let mut p = vec!["Flutter".into()];
                p.extend(platforms);
                p
            },
            version: main.version.clone(),
            build: main.build.clone(),
            source_of_truth: Some("pubspec.yaml".into()),
            targets,
            warnings,
            git: git::inspect(root),
            app_name: identity.app_name,
            bundle_id: identity.bundle_id,
        });
    }
    if let Some(file) = tauri_config {
        return scan_desktop_json(root, "tauri", "Tauri", &file, false, identity);
    }
    if electron_package.is_file() && is_electron_project(&electron_package)? {
        return scan_desktop_json(root, "electron", "Electron", &electron_package, true, identity);
    }
    if let Some(file) = gradle {
        let (version, build, safe) = parse_gradle(&file)?;
        if !safe {
            warnings.push(format!(
                "Android version values in {} are dynamically generated. Manual review required.",
                rel(root, &file)
            ));
        }
        targets.push(target(
            root,
            "android",
            "Android",
            &file,
            version,
            build,
            safe,
            false,
            if safe {
                None
            } else {
                Some("Manual review required".into())
            },
        ));
        platforms.push("Android".into());
    }
    if let Some(file) = pbx {
        let (version, build, safe) = parse_pbx(&file)?;
        if safe {
            targets.push(target(
                root, "ios", "iOS", &file, version, build, true, false, None,
            ));
        } else if let Some(info) = info_plists.first() {
            let (v, b, plist_safe) = parse_plist(info)?;
            targets.push(target(
                root,
                "ios",
                "iOS",
                info,
                v,
                b,
                plist_safe,
                false,
                Some(if plist_safe {
                    "Info.plist values".into()
                } else {
                    "Manual review required".into()
                }),
            ));
        } else {
            warnings.push(format!(
                "iOS values in {} are variable-based or ambiguous. Manual review required.",
                rel(root, &file)
            ));
            targets.push(target(
                root,
                "ios",
                "iOS",
                &file,
                None,
                None,
                false,
                false,
                Some("Manual review required".into()),
            ));
        }
        platforms.push("iOS".into());
    }
    if targets.is_empty() {
        warnings.push(
            "No supported Flutter, Android, Xcode, Electron, or Tauri version configuration was found.".into(),
        );
    }
    let first = targets.iter().find(|t| t.editable).or(targets.first());
    let project_type = match platforms.as_slice() {
        [one] => one.clone(),
        [_, _] => "Native iOS + Android".into(),
        _ => "Unknown".into(),
    };
    Ok(ProjectScan {
        root: root.to_string_lossy().into(),
        project_name: name(root),
        project_type,
        platforms,
        version: first.and_then(|t| t.version.clone()),
        build: first.and_then(|t| t.build.clone()),
        source_of_truth: first.map(|t| t.file.clone()),
        targets,
        warnings,
        git: git::inspect(root),
        app_name: identity.app_name,
        bundle_id: identity.bundle_id,
    })
}

fn scan_desktop_json(
    root: &Path,
    id: &str,
    platform: &str,
    file: &Path,
    supports_build_number: bool,
    identity: crate::rename::AppIdentity,
) -> Result<ProjectScan, String> {
    let (version, build, editable, note) = parse_desktop_json(file, supports_build_number)?;
    let mut warnings = Vec::new();
    if !editable {
        warnings.push(format!(
            "{} version in {} is not a static major.minor.patch value. Manual review required.",
            platform,
            rel(root, file)
        ));
    }
    let source_file = rel(root, file);
    let target_note = note.clone();
    let targets = vec![target(
        root,
        id,
        platform,
        file,
        version.clone(),
        build.clone(),
        editable,
        true,
        target_note,
    )];
    Ok(ProjectScan {
        root: root.to_string_lossy().into(),
        project_name: name(root),
        project_type: platform.into(),
        platforms: vec![platform.into()],
        version,
        build,
        source_of_truth: Some(source_file),
        targets,
        warnings,
        git: git::inspect(root),
        app_name: identity.app_name,
        bundle_id: identity.bundle_id,
    })
}

pub(crate) fn parse_desktop_json(
    file: &Path,
    supports_build_number: bool,
) -> Result<(Option<String>, Option<String>, bool, Option<String>), String> {
    let json: Value = serde_json::from_str(&read(file)?)
        .map_err(|error| format!("Could not parse {}: {error}", file.display()))?;
    let version = json.get("version").and_then(Value::as_str).map(str::to_owned);
    let editable = version.as_deref().map(valid_version).unwrap_or(false);
    let raw_build = supports_build_number.then(|| json.pointer("/build/buildNumber")).flatten();
    let build = raw_build.and_then(|value| match value {
        Value::String(value) if value.chars().all(|character| character.is_ascii_digit()) => Some(value.clone()),
        Value::Number(value) => value.as_u64().map(|value| value.to_string()),
        _ => None,
    });
    let note = if supports_build_number && raw_build.is_some() && build.is_none() {
        Some("build.buildNumber is dynamic; version can still be updated safely".into())
    } else if supports_build_number && raw_build.is_none() {
        Some("package.json version is the source of truth".into())
    } else {
        None
    };
    Ok((version.filter(|value| valid_version(value)), build, editable, note))
}

fn target(
    root: &Path,
    id: &str,
    platform: &str,
    file: &Path,
    version: Option<String>,
    build: Option<String>,
    editable: bool,
    source: bool,
    note: Option<String>,
) -> VersionTarget {
    VersionTarget {
        id: id.into(),
        platform: platform.into(),
        file: rel(root, file),
        version,
        build,
        editable,
        source_of_truth: source,
        note,
    }
}
pub(crate) fn parse_flutter(file: &Path) -> Result<(Option<String>, Option<String>), String> {
    let t = read(file)?;
    let re = Regex::new(r"(?m)^\s*version\s*:\s*([^\s#]+)").unwrap();
    let cap = re.captures(&t);
    let raw = cap.and_then(|c| c.get(1)).map(|m| m.as_str()).unwrap_or("");
    let mut p = raw.splitn(2, '+');
    let v = p.next().filter(|x| valid_version(x)).map(str::to_string);
    let b = p
        .next()
        .filter(|x| x.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string);
    Ok((v, b))
}
pub(crate) fn parse_gradle(file: &Path) -> Result<(Option<String>, Option<String>, bool), String> {
    let t = read(file)?;
    let (v, b, safe) = gradle_values(&t);
    Ok((v, b, safe))
}
pub fn gradle_values(t: &str) -> (Option<String>, Option<String>, bool) {
    let direct_name = Regex::new(r#"(?m)^\s*versionName\s*(?:=\s*)?[\"']([^\"']+)[\"']"#)
        .unwrap()
        .captures(t)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());
    let direct_code = Regex::new(r"(?m)^\s*versionCode\s*(?:=\s*)?(\d+)\b")
        .unwrap()
        .captures(t)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());
    let name = direct_name.or_else(|| simple_gradle_variable(t, "versionName", true));
    let code = direct_code.or_else(|| simple_gradle_variable(t, "versionCode", false));
    let safe = name.is_some() || code.is_some();
    (name, code, safe)
}
fn simple_gradle_variable(t: &str, property: &str, quoted: bool) -> Option<String> {
    let use_re = Regex::new(&format!(
        r"(?m)^\s*{}\s*(?:=\s*)?([A-Za-z_][A-Za-z0-9_]*)\s*$",
        property
    ))
    .ok()?;
    let variable = use_re.captures(t)?.get(1)?.as_str();
    let pattern = if quoted {
        format!(
            r#"(?m)^\s*(?:(?:def|val|final\s+def)\s+)?{}\s*=\s*[\"']([^\"']+)[\"']\s*$"#,
            regex::escape(variable)
        )
    } else {
        format!(
            r"(?m)^\s*(?:(?:def|val|final\s+def)\s+)?{}\s*=\s*(\d+)\s*$",
            regex::escape(variable)
        )
    };
    let re = Regex::new(&pattern).ok()?;
    let values: Vec<_> = re.captures_iter(t).filter_map(|c| c.get(1)).collect();
    if values.len() == 1 {
        Some(values[0].as_str().to_string())
    } else {
        None
    }
}
pub(crate) fn parse_pbx(file: &Path) -> Result<(Option<String>, Option<String>, bool), String> {
    let t = read(file)?;
    let vr = Regex::new(r"MARKETING_VERSION\s*=\s*([^;]+);").unwrap();
    let br = Regex::new(r"CURRENT_PROJECT_VERSION\s*=\s*([^;]+);").unwrap();
    let vals = |re: &Regex| -> Vec<String> {
        re.captures_iter(&t)
            .filter_map(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .collect()
    };
    let vs = vals(&vr);
    let bs = vals(&br);
    let safe = vs.iter().all(|v| valid_version(v))
        && bs.iter().all(|b| b.chars().all(|c| c.is_ascii_digit()))
        && !vs.is_empty()
        && !bs.is_empty();
    Ok((vs.first().cloned(), bs.first().cloned(), safe))
}
pub(crate) fn parse_plist(file: &Path) -> Result<(Option<String>, Option<String>, bool), String> {
    let t = read(file)?;
    let get = |key: &str| {
        let re = Regex::new(&format!(
            r"(?s)<key>{}</key>\s*<string>([^<]+)</string>",
            key
        ))
        .unwrap();
        re.captures(&t)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
    };
    let v = get("CFBundleShortVersionString");
    let b = get("CFBundleVersion");
    let safe = v.as_deref().map(valid_version).unwrap_or(false)
        && b.as_ref()
            .map(|x| x.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false);
    Ok((v, b, safe))
}
pub(crate) fn valid_version(v: &str) -> bool {
    let p: Vec<_> = v.split('.').collect();
    p.len() == 3
        && p.iter()
            .all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
}

fn recent_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("recent-projects.json"))
}
fn settings_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}
pub fn read_recents(app: &AppHandle) -> Result<Vec<RecentProject>, String> {
    let f = recent_file(app)?;
    if !f.exists() {
        return Ok(vec![]);
    };
    serde_json::from_str(&fs::read_to_string(f).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn store_recent(app: &AppHandle, root: &Path) -> Result<(), String> {
    let f = recent_file(app)?;
    let path = root.to_string_lossy().to_string();
    let mut items = read_recents(app)?;
    items.retain(|p| p.path != path);
    items.insert(
        0,
        RecentProject {
            path,
            name: name(root),
        },
    );
    items.truncate(8);
    fs::write(
        f,
        serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
pub fn remove_recent(app: &AppHandle, path: &str) -> Result<(), String> {
    let f = recent_file(app)?;
    let mut items = read_recents(app)?;
    items.retain(|project| project.path != path);
    fs::write(
        f,
        serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
pub fn load_settings(app: &AppHandle) -> Result<AppSettings, String> {
    let file = settings_file(app)?;
    if !file.exists() {
        return Ok(AppSettings::default());
    };
    serde_json::from_str(&fs::read_to_string(file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let file = settings_file(app)?;
    fs::write(
        file,
        serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
pub fn save_project_config(root: &Path) -> Result<(), String> {
    let s = scan(root)?;
    let config = serde_json::json!({"projectType":s.project_type.to_lowercase().replace(' ', "-"),"versionSource":s.source_of_truth,"targets":s.targets.iter().filter(|t|t.editable).map(|t|&t.file).collect::<Vec<_>>()});
    fs::write(
        root.join(".versionpilot.json"),
        serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};

    #[test]
    fn detects_static_electron_and_tauri_projects() {
        let base = env::temp_dir().join(format!("klenuabuild-desktop-test-{}", std::process::id()));
        let electron = base.join("electron");
        let tauri = base.join("tauri/src-tauri");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&electron).unwrap();
        fs::write(electron.join("package.json"), "{\"version\":\"1.4.2\",\"devDependencies\":{\"electron\":\"^31.0.0\"},\"build\":{\"buildNumber\":38}}").unwrap();
        fs::create_dir_all(&tauri).unwrap();
        fs::write(tauri.join("tauri.conf.json"), "{\"version\":\"2.3.4\"}").unwrap();

        let electron_scan = scan(&electron).unwrap();
        let tauri_scan = scan(&base.join("tauri")).unwrap();
        assert_eq!(electron_scan.project_type, "Electron");
        assert_eq!(electron_scan.version.as_deref(), Some("1.4.2"));
        assert_eq!(electron_scan.build.as_deref(), Some("38"));
        assert_eq!(tauri_scan.project_type, "Tauri");
        assert_eq!(tauri_scan.version.as_deref(), Some("2.3.4"));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn scan_reports_current_app_name_and_bundle_id() {
        let root = env::temp_dir().join(format!("klenuabuild-identity-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src-tauri")).unwrap();
        fs::write(
            root.join("src-tauri/tauri.conf.json"),
            "{\n  \"version\": \"2.3.4\",\n  \"productName\": \"My App\",\n  \"identifier\": \"com.acme.myapp\"\n}\n",
        )
        .unwrap();

        let scan = scan(&root).unwrap();
        assert_eq!(scan.app_name.as_deref(), Some("My App"));
        assert_eq!(scan.bundle_id.as_deref(), Some("com.acme.myapp"));
        let _ = fs::remove_dir_all(root);
    }
}
