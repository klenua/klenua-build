use crate::check::{manifest_label, parse_application_id, parse_bundle_identifier, plist_string};
use crate::core::change::{changed, FileChange, LineChange, Preview};
use crate::core::project;
use regex::Regex;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// What a project is currently called and identified as — read-only, no
/// preview or write involved. Used to report the up-to-date name/bundle ID
/// alongside version/build when syncing to a klenua account, so a rename
/// is reflected there without needing its own sync step.
pub struct AppIdentity {
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

pub fn current_identity(root: &Path) -> Result<AppIdentity, String> {
    let detected = project::detect(root)?;
    let mut app_name = None;
    let mut bundle_id = None;

    if let Some(gradle) = &detected.gradle {
        bundle_id = parse_application_id(&project::read(gradle)?);
    }
    if bundle_id.is_none() {
        if let Some(pbx) = &detected.pbx {
            bundle_id = parse_bundle_identifier(&project::read(pbx)?);
        }
    }
    if bundle_id.is_none() {
        if detected.electron_package_candidate.is_file() && project::is_electron_project(&detected.electron_package_candidate)? {
            bundle_id = read_json_field(&project::read(&detected.electron_package_candidate)?, "appId");
        }
    }
    if bundle_id.is_none() {
        if let Some(tauri_config) = &detected.tauri_config {
            bundle_id = read_json_field(&project::read(tauri_config)?, "identifier");
        }
    }

    let manifest = root.join("android/app/src/main/AndroidManifest.xml");
    if manifest.is_file() {
        app_name = manifest_label(&project::read(&manifest)?).filter(|label| !label.starts_with("@string/"));
    }
    if app_name.is_none() {
        if let Some(plist) = detected.info_plists.first() {
            let text = project::read(plist)?;
            app_name = plist_string(&text, "CFBundleDisplayName")
                .or_else(|| plist_string(&text, "CFBundleName"))
                .filter(|value| !value.contains("$("));
        }
    }
    if app_name.is_none() {
        if detected.electron_package_candidate.is_file() && project::is_electron_project(&detected.electron_package_candidate)? {
            app_name = read_json_field(&project::read(&detected.electron_package_candidate)?, "productName");
        }
    }
    if app_name.is_none() {
        if let Some(tauri_config) = &detected.tauri_config {
            app_name = read_json_field(&project::read(tauri_config)?, "productName");
        }
    }

    Ok(AppIdentity { app_name, bundle_id })
}

/// Read-only sibling of `rewrite_json_field`: the current value of a single
/// top-level (or one-level-nested) `"key": "..."` JSON string field, or
/// `None` if it's missing or ambiguous — never errors, since callers here
/// just want "best effort, whatever's there" rather than a rewrite target.
fn read_json_field(input: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"(?m)^\s*"{}"\s*:\s*"([^"]+)".*$"#, regex::escape(key))).unwrap();
    let values: Vec<_> = re.captures_iter(input).filter_map(|c| c.get(1)).map(|m| m.as_str().to_string()).collect();
    if values.len() == 1 {
        Some(values.into_iter().next().unwrap())
    } else {
        None
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManualAttention {
    pub file: String,
    pub reason: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreview {
    pub preview: Preview,
    pub warning: Option<String>,
    pub manual_attention: Vec<ManualAttention>,
    pub out_of_scope: Vec<String>,
}

const ID_OUT_OF_SCOPE: &[&str] = &[
    "Renaming the Dart package name in pubspec.yaml is not covered — update it manually if needed.",
    "Moving source directories to match a new package/bundle ID is not covered — move them manually if needed.",
];

/// Reads `file`, runs `rewrite`, and either records a `FileChange`, a
/// no-change warning, or (if `rewrite` errors — a field that's missing,
/// ambiguous, or dynamic) turns that error into a warning instead of
/// failing the whole preview, so one unsupported platform doesn't block
/// every other target from being renamed.
fn add_change(
    root: &Path,
    file: &Path,
    rewrite: impl Fn(&str) -> Result<(String, Vec<LineChange>), String>,
    changes: &mut Vec<FileChange>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let original = fs::read_to_string(file).map_err(|e| format!("Could not read {}: {e}", file.display()))?;
    match rewrite(&original) {
        Ok((updated, diffs)) if !diffs.is_empty() => {
            changes.push(FileChange {
                file: project::rel(root, file),
                changes: diffs,
                original,
                updated,
                absolute: file.to_path_buf(),
            });
        }
        Ok(_) => warnings.push(format!("No change is needed in {}.", project::rel(root, file))),
        Err(message) => warnings.push(format!("{}: {message}", project::rel(root, file))),
    }
    Ok(())
}

pub fn preview_name(root: &Path, new_name: &str) -> Result<RenamePreview, String> {
    let new_name = new_name.trim();
    if new_name.is_empty() {
        return Err("Enter a new display name.".into());
    }
    let detected = project::detect(root)?;
    let mut changes = Vec::new();
    let mut warnings = Vec::new();

    let manifest = root.join("android/app/src/main/AndroidManifest.xml");
    if manifest.is_file() {
        add_change(root, &manifest, |text| rewrite_android_label(text, new_name), &mut changes, &mut warnings)?;
    }
    if let Some(plist) = detected.info_plists.first() {
        add_change(root, plist, |text| rewrite_display_name_plist(text, new_name), &mut changes, &mut warnings)?;
    }
    if detected.electron_package_candidate.is_file() && project::is_electron_project(&detected.electron_package_candidate)? {
        add_change(root, &detected.electron_package_candidate, |text| rewrite_json_field(text, "productName", new_name), &mut changes, &mut warnings)?;
    }
    if let Some(tauri_config) = &detected.tauri_config {
        add_change(root, tauri_config, |text| rewrite_json_field(text, "productName", new_name), &mut changes, &mut warnings)?;
    }

    if changes.is_empty() {
        return Err("No safe display-name field was found. Manual review required.".into());
    }
    Ok(RenamePreview {
        preview: Preview { changes, warnings },
        warning: None,
        manual_attention: Vec::new(),
        out_of_scope: Vec::new(),
    })
}

pub fn preview_id(root: &Path, new_id: &str) -> Result<RenamePreview, String> {
    let new_id = new_id.trim();
    if new_id.is_empty() {
        return Err("Enter a new bundle/application ID.".into());
    }
    if !valid_bundle_id(new_id) {
        return Err("Bundle/application ID must look like a reverse-DNS identifier, e.g. com.company.app.".into());
    }
    let detected = project::detect(root)?;
    let mut changes = Vec::new();
    let mut warnings = Vec::new();
    let mut old_ids: Vec<String> = Vec::new();

    if let Some(gradle) = &detected.gradle {
        let gradle_text = project::read(gradle)?;
        match parse_application_id(&gradle_text) {
            Some(current) => {
                old_ids.push(current.clone());
                add_change(root, gradle, |text| rewrite_gradle_application_id(text, &current, new_id), &mut changes, &mut warnings)?;
                if let Some(main_activity) = find_main_activity(root) {
                    let source = project::read(&main_activity)?;
                    if package_declaration_matches(&source, &current) {
                        add_change(root, &main_activity, |text| rewrite_package_declaration(text, &current, new_id), &mut changes, &mut warnings)?;
                    }
                }
            }
            None => warnings.push(format!("No applicationId found in {}.", project::rel(root, gradle))),
        }
    }

    if let Some(pbx) = &detected.pbx {
        let pbx_text = project::read(pbx)?;
        if let Some(current) = parse_bundle_identifier(&pbx_text) {
            if !old_ids.contains(&current) {
                old_ids.push(current);
            }
        }
        add_change(root, pbx, |text| rewrite_pbx_bundle_id(text, new_id), &mut changes, &mut warnings)?;
    }

    if detected.electron_package_candidate.is_file() && project::is_electron_project(&detected.electron_package_candidate)? {
        add_change(root, &detected.electron_package_candidate, |text| rewrite_json_field(text, "appId", new_id), &mut changes, &mut warnings)?;
    }
    if let Some(tauri_config) = &detected.tauri_config {
        add_change(root, tauri_config, |text| rewrite_json_field(text, "identifier", new_id), &mut changes, &mut warnings)?;
    }

    if changes.is_empty() {
        return Err("No safe bundle/application ID field was found. Manual review required.".into());
    }

    let manual_attention = find_manual_attention_files(root, &old_ids)?;

    Ok(RenamePreview {
        preview: Preview { changes, warnings },
        warning: Some(
            "Changing the bundle/application ID makes this a different app in the App Store and Play Store — it will not update people who already installed the old one.".into(),
        ),
        manual_attention,
        out_of_scope: ID_OUT_OF_SCOPE.iter().map(|s| s.to_string()).collect(),
    })
}

fn rewrite_android_label(input: &str, new_name: &str) -> Result<(String, Vec<LineChange>), String> {
    let re = Regex::new(r#"android:label\s*=\s*"([^"]+)""#).unwrap();
    let cap = re.captures(input).ok_or("No android:label attribute found.")?;
    let old = cap.get(1).unwrap().as_str().to_string();
    if old.starts_with("@string/") {
        return Err(format!(
            "android:label references {old}, not a literal name; edit that string resource directly for now."
        ));
    }
    let mut diffs = vec![];
    changed(format!("android:label = \"{old}\""), format!("android:label = \"{new_name}\""), &mut diffs);
    let output = re.replace(input, format!("android:label=\"{new_name}\"")).to_string();
    Ok((output, diffs))
}

fn rewrite_display_name_plist(input: &str, new_name: &str) -> Result<(String, Vec<LineChange>), String> {
    let mut output = input.to_string();
    let mut diffs = vec![];
    let mut found_any = false;
    for key in ["CFBundleDisplayName", "CFBundleName"] {
        let re = Regex::new(&format!(r"(?s)(<key>{key}</key>\s*<string>)([^<]+)(</string>)")).unwrap();
        if let Some(cap) = re.captures(&output) {
            found_any = true;
            let old = cap.get(2).unwrap().as_str().trim().to_string();
            if old.contains("$(") {
                continue; // Xcode build-setting variable — leave it alone.
            }
            changed(format!("{key}: {old}"), format!("{key}: {new_name}"), &mut diffs);
            output = re.replace(&output, format!("${{1}}{new_name}${{3}}")).to_string();
        }
    }
    if !found_any {
        return Err("No CFBundleDisplayName or CFBundleName field found.".into());
    }
    Ok((output, diffs))
}

/// Shared by `productName` (Electron/Tauri) and `identifier`/`appId` — every
/// one of these is a single top-level (or one-level-nested) `"key": "..."`
/// JSON string field.
fn rewrite_json_field(input: &str, key: &str, new_value: &str) -> Result<(String, Vec<LineChange>), String> {
    let re = Regex::new(&format!(r#"(?m)^(\s*"{}"\s*:\s*")([^"]+)(".*)$"#, regex::escape(key))).unwrap();
    let values: Vec<_> = re.captures_iter(input).filter_map(|c| c.get(2)).map(|m| m.as_str().to_string()).collect();
    if values.len() != 1 {
        return Err(format!("No single \"{key}\" field found."));
    }
    let mut diffs = vec![];
    changed(format!("{key}: {}", values[0]), format!("{key}: {new_value}"), &mut diffs);
    let output = re.replace(input, format!("${{1}}{new_value}${{3}}")).to_string();
    Ok((output, diffs))
}

fn rewrite_gradle_application_id(input: &str, old: &str, new_id: &str) -> Result<(String, Vec<LineChange>), String> {
    let re = Regex::new(r#"(applicationId\s*(?:=\s*)?["'])([^"']+)(["'])"#).unwrap();
    let values: Vec<_> = re.captures_iter(input).filter_map(|c| c.get(2)).map(|m| m.as_str().to_string()).collect();
    if values.len() != 1 || values[0] != old {
        return Err("applicationId is not a single static value; manual review required.".into());
    }
    let mut diffs = vec![];
    changed(format!("applicationId \"{old}\""), format!("applicationId \"{new_id}\""), &mut diffs);
    let mut output = re.replace(input, format!("${{1}}{new_id}${{3}}")).to_string();

    let namespace_re = Regex::new(r#"(namespace\s*(?:=\s*)?["'])([^"']+)(["'])"#).unwrap();
    if let Some(cap) = namespace_re.captures(&output) {
        let namespace_old = cap.get(2).unwrap().as_str().to_string();
        if namespace_old == old {
            changed(format!("namespace \"{namespace_old}\""), format!("namespace \"{new_id}\""), &mut diffs);
            output = namespace_re.replace(&output, format!("${{1}}{new_id}${{3}}")).to_string();
        }
    }
    Ok((output, diffs))
}

fn rewrite_pbx_bundle_id(input: &str, new_id: &str) -> Result<(String, Vec<LineChange>), String> {
    let re = Regex::new(r"(PRODUCT_BUNDLE_IDENTIFIER\s*=\s*)([^;\s]+)(;)").unwrap();
    let values: Vec<_> = re.captures_iter(input).filter_map(|c| c.get(2)).map(|m| m.as_str().trim().to_string()).collect();
    if values.is_empty() {
        return Err("No PRODUCT_BUNDLE_IDENTIFIER found.".into());
    }
    if values.iter().any(|v| v != &values[0]) {
        return Err("PRODUCT_BUNDLE_IDENTIFIER differs across build configurations; manual review required.".into());
    }
    let mut diffs = vec![];
    for old in values.iter().take(3) {
        changed(format!("PRODUCT_BUNDLE_IDENTIFIER = {old};"), format!("PRODUCT_BUNDLE_IDENTIFIER = {new_id};"), &mut diffs);
    }
    let output = re.replace_all(input, format!("${{1}}{new_id}${{3}}")).to_string();
    Ok((output, diffs))
}

fn find_main_activity(root: &Path) -> Option<PathBuf> {
    for candidate in ["android/app/src/main/kotlin", "android/app/src/main/java"] {
        let dir = root.join(candidate);
        if let Some(found) = find_file_named(&dir, "MainActivity.kt").or_else(|| find_file_named(&dir, "MainActivity.java")) {
            return Some(found);
        }
    }
    None
}

fn find_file_named(dir: &Path, name: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_named(&path, name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(path);
        }
    }
    None
}

fn package_declaration_matches(text: &str, old_id: &str) -> bool {
    Regex::new(&format!(r"(?m)^\s*package\s+{}\s*$", regex::escape(old_id))).unwrap().is_match(text)
}

fn rewrite_package_declaration(input: &str, old: &str, new_id: &str) -> Result<(String, Vec<LineChange>), String> {
    let re = Regex::new(&format!(r"(?m)^(\s*package\s+){}(\s*)$", regex::escape(old))).unwrap();
    if !re.is_match(input) {
        return Err("MainActivity package declaration no longer matches the old application ID.".into());
    }
    let mut diffs = vec![];
    changed(format!("package {old}"), format!("package {new_id}"), &mut diffs);
    let output = re.replace(input, format!("${{1}}{new_id}${{2}}")).to_string();
    Ok((output, diffs))
}

fn valid_bundle_id(id: &str) -> bool {
    let parts: Vec<_> = id.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

const TEXT_EXTENSIONS: &[&str] = &[
    "json", "plist", "xml", "gradle", "kts", "entitlements", "yaml", "yml", "toml", "pbxproj",
    "swift", "kt", "java", "h", "m", "strings", "properties", "xcscheme", "xcconfig",
];

fn is_probably_text_file(file: &Path) -> bool {
    file.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| TEXT_EXTENSIONS.contains(&extension.to_lowercase().as_str()))
        .unwrap_or(false)
        && fs::metadata(file).map(|metadata| metadata.len() < 2_000_000).unwrap_or(false)
}

/// Never modifies anything — just flags files a bundle-ID rename can't
/// safely rewrite on its own: known Firebase/Google Services config files,
/// entitlements, and (as a catch-all for "anything else that references the
/// old ID", including URL schemes and deep links) any other small text file
/// that still contains one of the old IDs after the safe rewrites above.
fn find_manual_attention_files(root: &Path, old_ids: &[String]) -> Result<Vec<ManualAttention>, String> {
    let mut results = Vec::new();
    for file in project::walk(root)? {
        let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name == "google-services.json" || file_name == "GoogleService-Info.plist" {
            results.push(ManualAttention {
                file: project::rel(root, &file),
                reason: "References your Firebase/Google Services app — regenerate or update it for the new ID.".into(),
            });
            continue;
        }
        if file_name.ends_with(".entitlements") {
            results.push(ManualAttention {
                file: project::rel(root, &file),
                reason: "Entitlements file — check it doesn't hardcode the old bundle ID (e.g. an App Group or Keychain Sharing identifier).".into(),
            });
            continue;
        }
        if is_probably_text_file(&file) {
            if let Ok(text) = fs::read_to_string(&file) {
                if old_ids.iter().any(|id| text.contains(id.as_str())) {
                    results.push(ManualAttention {
                        file: project::rel(root, &file),
                        reason: "Still contains the old ID — check whether it needs updating (e.g. a URL scheme or deep link).".into(),
                    });
                }
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::change;
    use std::env;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("klenua-rename-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn renames_display_name_across_android_ios_and_tauri() {
        let root = temp_dir("display-name");
        fs::create_dir_all(root.join("android/app/src/main")).unwrap();
        fs::write(
            root.join("android/app/src/main/AndroidManifest.xml"),
            "<manifest><application android:label=\"Old Name\"></application></manifest>",
        )
        .unwrap();
        let ios_dir = root.join("ios/Runner");
        fs::create_dir_all(&ios_dir).unwrap();
        fs::write(
            ios_dir.join("Info.plist"),
            "<plist><dict><key>CFBundleDisplayName</key><string>Old Name</string><key>CFBundleName</key><string>Old</string></dict></plist>",
        )
        .unwrap();
        fs::create_dir_all(root.join("src-tauri")).unwrap();
        fs::write(root.join("src-tauri/tauri.conf.json"), "{\n  \"productName\": \"Old Name\"\n}\n").unwrap();

        let preview = preview_name(&root, "New Name").unwrap();
        assert_eq!(preview.preview.changes.len(), 3);
        change::apply(&root, preview.preview).unwrap();

        assert!(fs::read_to_string(root.join("android/app/src/main/AndroidManifest.xml")).unwrap().contains("android:label=\"New Name\""));
        assert!(fs::read_to_string(ios_dir.join("Info.plist")).unwrap().contains("<string>New Name</string>"));
        assert!(fs::read_to_string(root.join("src-tauri/tauri.conf.json")).unwrap().contains("\"productName\": \"New Name\""));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_indirect_android_label_reference() {
        let root = temp_dir("indirect-label");
        fs::create_dir_all(root.join("android/app/src/main")).unwrap();
        fs::write(
            root.join("android/app/src/main/AndroidManifest.xml"),
            "<manifest><application android:label=\"@string/app_name\"></application></manifest>",
        )
        .unwrap();
        let result = preview_name(&root, "New Name");
        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn renames_bundle_id_and_matching_namespace_and_main_activity() {
        let root = temp_dir("bundle-id");
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(
            root.join("android/app/build.gradle"),
            "android {\n  namespace \"com.old.app\"\n  defaultConfig { applicationId \"com.old.app\" }\n}",
        )
        .unwrap();
        let kotlin_dir = root.join("android/app/src/main/kotlin/com/old/app");
        fs::create_dir_all(&kotlin_dir).unwrap();
        fs::write(kotlin_dir.join("MainActivity.kt"), "package com.old.app\n\nclass MainActivity\n").unwrap();

        let preview = preview_id(&root, "com.new.app").unwrap();
        assert_eq!(preview.preview.changes.len(), 2);
        assert!(preview.warning.is_some());
        change::apply(&root, preview.preview).unwrap();

        let gradle_text = fs::read_to_string(root.join("android/app/build.gradle")).unwrap();
        assert!(gradle_text.contains("applicationId \"com.new.app\""));
        assert!(gradle_text.contains("namespace \"com.new.app\""));
        assert!(fs::read_to_string(kotlin_dir.join("MainActivity.kt")).unwrap().contains("package com.new.app"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn flags_google_services_json_for_manual_attention() {
        let root = temp_dir("manual-attention");
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(
            root.join("android/app/build.gradle"),
            "android { defaultConfig { applicationId \"com.old.app\" } }",
        )
        .unwrap();
        fs::write(root.join("android/app/google-services.json"), "{\"project_info\":{}}").unwrap();

        let preview = preview_id(&root, "com.new.app").unwrap();
        assert!(preview.manual_attention.iter().any(|m| m.file.ends_with("google-services.json")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_a_malformed_bundle_id() {
        let root = temp_dir("bad-id");
        fs::create_dir_all(&root).unwrap();
        assert!(preview_id(&root, "not-a-bundle-id").is_err());
        let _ = fs::remove_dir_all(root);
    }
}
