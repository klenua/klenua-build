use crate::core::project;
use crate::scanner;
use regex::Regex;
use serde::Serialize;
use std::path::Path;

mod permissions;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CheckFinding {
    pub severity: String, // "error" | "warning" | "info"
    pub rule: String,
    pub file: String,
    pub message: String,
    pub fix_hint: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub findings: Vec<CheckFinding>,
    pub has_errors: bool,
}

struct PlatformVersion {
    label: &'static str,
    file: String,
    version: Option<String>,
    build: Option<String>,
}

fn finding(severity: &str, rule: &str, file: impl Into<String>, message: impl Into<String>, fix_hint: impl Into<String>) -> CheckFinding {
    CheckFinding {
        severity: severity.into(),
        rule: rule.into(),
        file: file.into(),
        message: message.into(),
        fix_hint: fix_hint.into(),
    }
}

pub fn check(root: &Path) -> Result<CheckReport, String> {
    let detected = project::detect(root)?;
    let mut findings = Vec::new();

    let mut platforms: Vec<PlatformVersion> = Vec::new();
    if detected.pubspec.is_file() {
        let (version, build) = scanner::parse_flutter(&detected.pubspec)?;
        platforms.push(PlatformVersion { label: "Flutter", file: project::rel(root, &detected.pubspec), version, build });
    }
    if let Some(gradle) = &detected.gradle {
        let (version, build, _safe) = scanner::parse_gradle(gradle)?;
        platforms.push(PlatformVersion { label: "Android", file: project::rel(root, gradle), version, build });
    }
    if let Some(pbx) = &detected.pbx {
        let (version, build, safe) = scanner::parse_pbx(pbx)?;
        if safe {
            platforms.push(PlatformVersion { label: "iOS/macOS", file: project::rel(root, pbx), version, build });
        } else if let Some(plist) = detected.info_plists.first() {
            let (version, build, _safe) = scanner::parse_plist(plist)?;
            platforms.push(PlatformVersion { label: "iOS/macOS", file: project::rel(root, plist), version, build });
        }
    }
    if let Some(tauri_config) = &detected.tauri_config {
        let (version, _build, _editable, _note) = scanner::parse_desktop_json(tauri_config, false)?;
        platforms.push(PlatformVersion { label: "Tauri", file: project::rel(root, tauri_config), version, build: None });
    } else if detected.electron_package_candidate.is_file()
        && project::is_electron_project(&detected.electron_package_candidate)?
    {
        let (version, build, _editable, _note) = scanner::parse_desktop_json(&detected.electron_package_candidate, true)?;
        platforms.push(PlatformVersion { label: "Electron", file: project::rel(root, &detected.electron_package_candidate), version, build });
    }

    check_version_consistency(&platforms, &mut findings);
    check_valid_formats(&platforms, &detected, root, &mut findings);
    check_placeholders(&detected, &mut findings, root);
    check_ios_usage_descriptions(&detected, root, &mut findings)?;
    check_bundle_id_consistency(&detected, root, &mut findings)?;
    check_release_signing(&detected, root, &mut findings)?;

    let has_errors = findings.iter().any(|f| f.severity == "error");
    Ok(CheckReport { findings, has_errors })
}

fn check_version_consistency(platforms: &[PlatformVersion], findings: &mut Vec<CheckFinding>) {
    let versions: Vec<&str> = platforms.iter().filter_map(|p| p.version.as_deref()).collect();
    if versions.iter().any(|v| Some(v) != versions.first()) {
        let detail = platforms
            .iter()
            .filter_map(|p| p.version.as_deref().map(|v| format!("{}: {v}", p.label)))
            .collect::<Vec<_>>()
            .join(", ");
        findings.push(finding(
            "error",
            "version-consistency",
            platforms.iter().map(|p| p.file.as_str()).collect::<Vec<_>>().join(", "),
            format!("Version differs across platforms ({detail})."),
            "Bring every platform to the same version, or confirm the source-of-truth platform is intentionally ahead.",
        ));
    }
    let builds: Vec<&str> = platforms.iter().filter_map(|p| p.build.as_deref()).collect();
    if builds.iter().any(|b| Some(b) != builds.first()) {
        let detail = platforms
            .iter()
            .filter_map(|p| p.build.as_deref().map(|b| format!("{}: {b}", p.label)))
            .collect::<Vec<_>>()
            .join(", ");
        findings.push(finding(
            "error",
            "version-consistency",
            platforms.iter().map(|p| p.file.as_str()).collect::<Vec<_>>().join(", "),
            format!("Build number differs across platforms ({detail})."),
            "Bring every platform to the same build number before release.",
        ));
    }
}

fn check_valid_formats(platforms: &[PlatformVersion], detected: &project::DetectedFiles, root: &Path, findings: &mut Vec<CheckFinding>) {
    for platform in platforms {
        if let Some(version) = &platform.version {
            if !scanner::valid_version(version) {
                findings.push(finding(
                    "error",
                    "valid-formats",
                    &platform.file,
                    format!("{} version \"{version}\" is not a valid major.minor.patch value.", platform.label),
                    "Use a semantic version like 1.2.3.",
                ));
            }
        }
    }
    if let Some(gradle) = &detected.gradle {
        if let Some(android) = platforms.iter().find(|p| p.label == "Android") {
            if let Some(build) = &android.build {
                match build.parse::<u64>() {
                    Ok(code) if code >= 1 && code <= 2_100_000_000 => {}
                    _ => findings.push(finding(
                        "error",
                        "valid-formats",
                        project::rel(root, gradle),
                        format!("Android versionCode \"{build}\" is not an integer between 1 and 2100000000."),
                        "Set versionCode to a whole number in the range Google Play accepts.",
                    )),
                }
            }
        }
    }
    if let Some(ios) = platforms.iter().find(|p| p.label == "iOS/macOS") {
        if let Some(build) = &ios.build {
            let re = Regex::new(r"^\d+(\.\d+){0,2}$").unwrap();
            if !re.is_match(build) {
                findings.push(finding(
                    "error",
                    "valid-formats",
                    &ios.file,
                    format!("iOS/macOS build number \"{build}\" is not a valid CFBundleVersion (1 to 3 dot-separated integers)."),
                    "Use a format like 1, 1.2, or 1.2.3.",
                ));
            }
        }
    }
}

fn check_placeholders(detected: &project::DetectedFiles, findings: &mut Vec<CheckFinding>, root: &Path) {
    if let Some(gradle) = &detected.gradle {
        if let Ok(text) = project::read(gradle) {
            if let Some(id) = parse_application_id(&text) {
                if id.contains("com.example") {
                    findings.push(finding(
                        "warning",
                        "placeholders",
                        project::rel(root, gradle),
                        format!("Android applicationId \"{id}\" still uses the com.example placeholder."),
                        "Set applicationId to your real, registered application ID.",
                    ));
                }
            }
        }
    }
    if let Some(pbx) = &detected.pbx {
        if let Ok(text) = project::read(pbx) {
            if let Some(id) = parse_bundle_identifier(&text) {
                if id.contains("com.example") {
                    findings.push(finding(
                        "warning",
                        "placeholders",
                        project::rel(root, pbx),
                        format!("iOS/macOS PRODUCT_BUNDLE_IDENTIFIER \"{id}\" still uses the com.example placeholder."),
                        "Set PRODUCT_BUNDLE_IDENTIFIER to your real, registered bundle ID.",
                    ));
                }
            }
        }
    }
    if let Some(plist) = detected.info_plists.first() {
        if let Ok(text) = project::read(plist) {
            let display_name = plist_string(&text, "CFBundleDisplayName").or_else(|| plist_string(&text, "CFBundleName"));
            if display_name.as_deref() == Some("Runner") {
                findings.push(finding(
                    "warning",
                    "placeholders",
                    project::rel(root, plist),
                    "App display name is still the Flutter/Xcode template default \"Runner\".",
                    "Set CFBundleDisplayName to your real app name, or use klenua rename.",
                ));
            }
        }
    }
    let manifest = root.join("android/app/src/main/AndroidManifest.xml");
    if manifest.is_file() {
        if let Ok(text) = project::read(&manifest) {
            if let Some(label) = manifest_label(&text) {
                if label == "My Application" {
                    findings.push(finding(
                        "warning",
                        "placeholders",
                        project::rel(root, &manifest),
                        "App display name is still the Android Studio template default \"My Application\".",
                        "Set android:label to your real app name, or use klenua rename.",
                    ));
                }
            }
        }
    }
}

/// Heuristic, not a permission audit: a listed plugin doesn't guarantee the
/// permission is used at runtime, and this only covers the plugins in
/// permissions.rs. Labelled as a heuristic in every finding it produces.
fn check_ios_usage_descriptions(detected: &project::DetectedFiles, root: &Path, findings: &mut Vec<CheckFinding>) -> Result<(), String> {
    if !detected.pubspec.is_file() {
        return Ok(());
    }
    let Some(plist) = detected.info_plists.first() else { return Ok(()) };
    let dependencies = parse_pubspec_dependencies(&detected.pubspec)?;
    let plist_text = project::read(plist)?;
    for rule in permissions::IOS_PLUGIN_PERMISSIONS {
        let matched = dependencies.iter().any(|dep| dep.contains(rule.plugin));
        if matched && !plist_text.contains(rule.info_plist_key) {
            findings.push(finding(
                "warning",
                "ios-usage-descriptions",
                project::rel(root, plist),
                format!(
                    "Heuristic: pubspec.yaml depends on a plugin matching \"{}\", which usually needs {} — but Info.plist has no {} key.",
                    rule.plugin, rule.label, rule.info_plist_key
                ),
                format!("Add {} to Info.plist if your app actually requests {}.", rule.info_plist_key, rule.label),
            ));
        }
    }
    Ok(())
}

fn check_bundle_id_consistency(detected: &project::DetectedFiles, root: &Path, findings: &mut Vec<CheckFinding>) -> Result<(), String> {
    let android_id = match &detected.gradle {
        Some(gradle) => parse_application_id(&project::read(gradle)?),
        None => None,
    };
    let ios_id = match &detected.pbx {
        Some(pbx) => parse_bundle_identifier(&project::read(pbx)?),
        None => None,
    };
    if let (Some(android_id), Some(ios_id)) = (&android_id, &ios_id) {
        if android_id != ios_id {
            let files = [
                detected.gradle.as_ref().map(|p| project::rel(root, p)),
                detected.pbx.as_ref().map(|p| project::rel(root, p)),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
            findings.push(finding(
                "info",
                "bundle-id-consistency",
                files,
                format!("Android applicationId (\"{android_id}\") differs from iOS PRODUCT_BUNDLE_IDENTIFIER (\"{ios_id}\")."),
                "This can be intentional (different platform bundle IDs) — verify it's not a mistake.",
            ));
        }
    }
    Ok(())
}

fn check_release_signing(detected: &project::DetectedFiles, root: &Path, findings: &mut Vec<CheckFinding>) -> Result<(), String> {
    let Some(gradle) = &detected.gradle else { return Ok(()) };
    let text = project::read(gradle)?;
    if let Some(release_block) = release_build_type_block(&text) {
        if release_block.contains("signingConfigs.debug") {
            findings.push(finding(
                "warning",
                "signing",
                project::rel(root, gradle),
                "The release build type still uses the debug signing config.",
                "Configure a release signing config before publishing a release build.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn parse_application_id(text: &str) -> Option<String> {
    Regex::new(r#"applicationId\s*(?:=\s*)?["']([^"']+)["']"#)
        .unwrap()
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

pub(crate) fn parse_bundle_identifier(text: &str) -> Option<String> {
    Regex::new(r"PRODUCT_BUNDLE_IDENTIFIER\s*=\s*([^;\s]+);")
        .unwrap()
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

pub(crate) fn plist_string(text: &str, key: &str) -> Option<String> {
    Regex::new(&format!(r"(?s)<key>{}</key>\s*<string>([^<]+)</string>", regex::escape(key)))
        .unwrap()
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

pub(crate) fn manifest_label(text: &str) -> Option<String> {
    Regex::new(r#"android:label\s*=\s*"([^"]+)""#)
        .unwrap()
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Naive single-level brace match for the `release { ... }` block inside
/// `buildTypes { ... }` — good enough for the common, flat Gradle shape;
/// deeply nested or multi-block release configs fall through undetected
/// rather than risk a false positive from mismatched braces. The block's
/// start is anchored to `release` as its own line-leading identifier so it
/// can't latch onto a substring match inside an unrelated word like
/// "prerelease" earlier in the file.
fn release_build_type_block(text: &str) -> Option<&str> {
    let start = Regex::new(r"(?m)^\s*release\s*\{").unwrap().find(text)?.start();
    let brace_start = text[start..].find('{')? + start;
    let brace_end = text[brace_start..].find('}')? + brace_start;
    Some(&text[brace_start..=brace_end])
}

fn parse_pubspec_dependencies(file: &Path) -> Result<Vec<String>, String> {
    let text = project::read(file)?;
    let mut deps = Vec::new();
    let mut in_dependencies = false;
    for line in text.lines() {
        if line.starts_with("dependencies:") {
            in_dependencies = true;
            continue;
        }
        if in_dependencies {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if !line.starts_with(' ') {
                break; // next top-level key
            }
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if indent != 2 {
                continue; // nested value (e.g. version constraint), not a dependency name
            }
            if let Some((name, _)) = trimmed.split_once(':') {
                deps.push(name.trim().to_string());
            }
        }
    }
    Ok(deps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = env::temp_dir().join(format!("klenua-check-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn flags_version_mismatch_between_flutter_and_android() {
        let root = temp_dir("version-mismatch");
        fs::write(root.join("pubspec.yaml"), "name: demo\nversion: 1.5.0+40\n").unwrap();
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(
            root.join("android/app/build.gradle"),
            "android { defaultConfig { versionName \"1.4.0\"\nversionCode 39 } }",
        )
        .unwrap();
        let report = check(&root).unwrap();
        assert!(report.has_errors);
        assert!(report.findings.iter().any(|f| f.rule == "version-consistency"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn flags_invalid_android_version_code_range() {
        let root = temp_dir("bad-version-code");
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(
            root.join("android/app/build.gradle"),
            "android { defaultConfig { versionName \"1.0.0\"\nversionCode 0 } }",
        )
        .unwrap();
        let report = check(&root).unwrap();
        assert!(report.findings.iter().any(|f| f.rule == "valid-formats" && f.message.contains("versionCode")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn flags_com_example_placeholder_application_id() {
        let root = temp_dir("placeholder");
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(
            root.join("android/app/build.gradle"),
            "android { defaultConfig { applicationId \"com.example.demo\"\nversionName \"1.0.0\"\nversionCode 1 } }",
        )
        .unwrap();
        let report = check(&root).unwrap();
        assert!(report.findings.iter().any(|f| f.rule == "placeholders" && f.message.contains("com.example")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn flags_missing_camera_usage_description_when_camera_plugin_present() {
        let root = temp_dir("ios-usage");
        fs::write(root.join("pubspec.yaml"), "name: demo\nversion: 1.0.0+1\ndependencies:\n  flutter:\n    sdk: flutter\n  camera: ^0.10.0\n").unwrap();
        let ios_dir = root.join("ios/Runner");
        fs::create_dir_all(&ios_dir).unwrap();
        fs::write(
            ios_dir.join("Info.plist"),
            "<plist><dict><key>CFBundleShortVersionString</key><string>1.0.0</string><key>CFBundleVersion</key><string>1</string></dict></plist>",
        )
        .unwrap();
        let report = check(&root).unwrap();
        assert!(report
            .findings
            .iter()
            .any(|f| f.rule == "ios-usage-descriptions" && f.message.contains("NSCameraUsageDescription")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn flags_release_build_type_using_debug_signing() {
        let root = temp_dir("debug-signing");
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(
            root.join("android/app/build.gradle"),
            "android {\n  defaultConfig { applicationId \"com.acme.demo\"\nversionName \"1.0.0\"\nversionCode 1 }\n  buildTypes {\n    release {\n      signingConfig signingConfigs.debug\n    }\n  }\n}",
        )
        .unwrap();
        let report = check(&root).unwrap();
        assert!(report.findings.iter().any(|f| f.rule == "signing"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn no_findings_for_a_clean_consistent_project() {
        let root = temp_dir("clean");
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(
            root.join("android/app/build.gradle"),
            "android {\n  defaultConfig { applicationId \"com.acme.demo\"\nversionName \"1.0.0\"\nversionCode 1 }\n  buildTypes {\n    release {\n      signingConfig signingConfigs.release\n    }\n  }\n}",
        )
        .unwrap();
        let report = check(&root).unwrap();
        assert!(!report.has_errors);
        assert!(report.findings.is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
