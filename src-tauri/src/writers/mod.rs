use crate::{backup, scanner};
use regex::Regex;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LineChange {
    pub before: String,
    pub after: String,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub file: String,
    pub changes: Vec<LineChange>,
    #[serde(skip)]
    pub original: String,
    #[serde(skip)]
    pub updated: String,
    #[serde(skip)]
    pub absolute: PathBuf,
}
#[derive(Serialize, Clone)]
pub struct Preview {
    pub changes: Vec<FileChange>,
    pub warnings: Vec<String>,
}

pub fn preview(root: &Path, version: &str, build: &str) -> Result<Preview, String> {
    if !version.is_empty() && !valid_version(version) {
        return Err("Version must use major.minor.patch, for example 1.5.0.".into());
    }
    if !build.is_empty() && !build.chars().all(|c| c.is_ascii_digit()) {
        return Err("Build must be a whole number.".into());
    }
    let scan = scanner::scan(root)?;
    let mut changes = Vec::new();
    let mut warnings = scan.warnings;
    for target in scan.targets.iter().filter(|t| t.editable) {
        let file = root.join(&target.file);
        let original = fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let (updated, diffs) = match target.id.as_str() {
            "flutter" => rewrite_flutter(&original, version, build),
            "android" => rewrite_gradle(&original, version, build),
            "ios" => {
                if target.file.ends_with(".plist") {
                    rewrite_plist(&original, version, build)
                } else {
                    rewrite_pbx(&original, version, build)
                }
            }
            "electron" => rewrite_desktop_json(&original, version, build, "Electron", true),
            "tauri" => rewrite_desktop_json(&original, version, build, "Tauri", false),
            _ => Ok((original.clone(), vec![])),
        }?;
        if diffs.is_empty() {
            warnings.push(format!("No change is needed in {}.", target.file));
        } else {
            changes.push(FileChange {
                file: target.file.clone(),
                changes: diffs,
                original,
                updated,
                absolute: file,
            });
        }
    }
    if changes.is_empty() {
        warnings.push(
            "No safe file update was prepared. Manual review required if values are dynamic."
                .into(),
        );
    }
    Ok(Preview { changes, warnings })
}
pub fn apply(root: &Path, preview: Preview) -> Result<(), String> {
    for file in preview.changes {
        backup::save(root, &file.absolute, &file.original)?;
        fs::write(&file.absolute, file.updated)
            .map_err(|e| format!("Could not write {}: {}", file.file, e))?;
    }
    Ok(())
}

fn changed(before: String, after: String, d: &mut Vec<LineChange>) {
    if before != after {
        d.push(LineChange { before, after })
    }
}
fn rewrite_flutter(input: &str, v: &str, b: &str) -> Result<(String, Vec<LineChange>), String> {
    let re = Regex::new(r"(?m)^(\s*version\s*:\s*)([^\s#]+)(.*)$").unwrap();
    let cap = re
        .captures(input)
        .ok_or("Could not safely locate the Flutter version field.")?;
    let old = cap.get(2).unwrap().as_str();
    let mut parts = old.splitn(2, '+');
    let old_v = parts.next().unwrap_or("");
    let old_b = parts.next().unwrap_or("");
    if !valid_version(old_v) || (!old_b.is_empty() && !numeric(old_b)) {
        return Err("Flutter version field is not a safe static value.".into());
    }
    let new_v = if v.is_empty() { old_v } else { v };
    let new_b = if b.is_empty() { old_b } else { b };
    let next = if new_b.is_empty() {
        new_v.to_string()
    } else {
        format!("{}+{}", new_v, new_b)
    };
    let mut diffs = vec![];
    changed(
        format!("version: {}", old),
        format!("version: {}", next),
        &mut diffs,
    );
    let output = re
        .replace(input, format!("${{1}}{}${{3}}", next))
        .to_string();
    Ok((output, diffs))
}
fn rewrite_gradle(input: &str, v: &str, b: &str) -> Result<(String, Vec<LineChange>), String> {
    let mut output = input.to_string();
    let mut diffs = vec![];
    if !v.is_empty() {
        let re =
            Regex::new(r#"(?m)^(\s*versionName\s*(?:=\s*)?[\"'])([^\"']+)([\"'].*)$"#).unwrap();
        if let Some(c) = re.captures(&output) {
            let old = c.get(2).unwrap().as_str().to_string();
            changed(
                format!("versionName = \"{}\"", old),
                format!("versionName = \"{}\"", v),
                &mut diffs,
            );
            output = re
                .replace_all(&output, format!("${{1}}{}${{3}}", v))
                .to_string();
        } else {
            output = rewrite_gradle_variable(&output, "versionName", v, true, &mut diffs)?
        }
    }
    if !b.is_empty() {
        let re = Regex::new(r"(?m)^(\s*versionCode\s*(?:=\s*)?)(\d+)(.*)$").unwrap();
        if let Some(c) = re.captures(&output) {
            let old = c.get(2).unwrap().as_str().to_string();
            changed(
                format!("versionCode = {}", old),
                format!("versionCode = {}", b),
                &mut diffs,
            );
            output = re
                .replace_all(&output, format!("${{1}}{}${{3}}", b))
                .to_string();
        } else {
            output = rewrite_gradle_variable(&output, "versionCode", b, false, &mut diffs)?
        }
    }
    Ok((output, diffs))
}
fn rewrite_gradle_variable(
    input: &str,
    property: &str,
    new_value: &str,
    quoted: bool,
    diffs: &mut Vec<LineChange>,
) -> Result<String, String> {
    let use_re = Regex::new(&format!(
        r"(?m)^\s*{}\s*(?:=\s*)?([A-Za-z_][A-Za-z0-9_]*)\s*$",
        property
    ))
    .unwrap();
    let token = use_re
        .captures(input)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or("Android version value is dynamic or missing; manual review required.")?;
    let pattern = if quoted {
        format!(
            r#"(?m)^(\s*(?:(?:def|val|final\s+def)\s+)?{}\s*=\s*[\"'])([^\"']+)([\"'].*)$"#,
            regex::escape(&token)
        )
    } else {
        format!(
            r"(?m)^(\s*(?:(?:def|val|final\s+def)\s+)?{}\s*=\s*)(\d+)(.*)$",
            regex::escape(&token)
        )
    };
    let re = Regex::new(&pattern).unwrap();
    let values: Vec<_> = re
        .captures_iter(input)
        .filter_map(|c| c.get(2))
        .map(|m| m.as_str().to_string())
        .collect();
    if values.len() != 1 {
        return Err(
            "Android variable is not a single static definition; manual review required.".into(),
        );
    }
    let old = &values[0];
    changed(
        format!("{} = {}", token, old),
        format!("{} = {}", token, new_value),
        diffs,
    );
    Ok(re
        .replace(input, format!("${{1}}{}${{3}}", new_value))
        .to_string())
}
fn rewrite_pbx(input: &str, v: &str, b: &str) -> Result<(String, Vec<LineChange>), String> {
    let mut output = input.to_string();
    let mut diffs = vec![];
    if !v.is_empty() {
        let re = Regex::new(r"(?m)(MARKETING_VERSION\s*=\s*)([^;\s]+)(;)").unwrap();
        let values: Vec<_> = re
            .captures_iter(&output)
            .filter_map(|c| c.get(2))
            .map(|m| m.as_str().to_string())
            .collect();
        if values.is_empty() || values.iter().any(|x| !valid_version(x)) {
            return Err(
                "Xcode MARKETING_VERSION is variable-based or ambiguous; manual review required."
                    .into(),
            );
        }
        for old in values.iter().take(3) {
            changed(
                format!("MARKETING_VERSION = {};", old),
                format!("MARKETING_VERSION = {};", v),
                &mut diffs,
            )
        }
        output = re
            .replace_all(&output, format!("${{1}}{}${{3}}", v))
            .to_string();
    }
    if !b.is_empty() {
        let re = Regex::new(r"(?m)(CURRENT_PROJECT_VERSION\s*=\s*)([^;\s]+)(;)").unwrap();
        let values: Vec<_> = re
            .captures_iter(&output)
            .filter_map(|c| c.get(2))
            .map(|m| m.as_str().to_string())
            .collect();
        if values.is_empty() || values.iter().any(|x| !numeric(x)) {
            return Err("Xcode CURRENT_PROJECT_VERSION is variable-based or ambiguous; manual review required.".into());
        }
        for old in values.iter().take(3) {
            changed(
                format!("CURRENT_PROJECT_VERSION = {};", old),
                format!("CURRENT_PROJECT_VERSION = {};", b),
                &mut diffs,
            )
        }
        output = re
            .replace_all(&output, format!("${{1}}{}${{3}}", b))
            .to_string();
    }
    Ok((output, diffs))
}
fn rewrite_plist(input: &str, v: &str, b: &str) -> Result<(String, Vec<LineChange>), String> {
    let mut output = input.to_string();
    let mut diffs = vec![];
    for (key, new_value) in [("CFBundleShortVersionString", v), ("CFBundleVersion", b)] {
        if new_value.is_empty() {
            continue;
        }
        let re = Regex::new(&format!(
            r"(?s)(<key>{}</key>\s*<string>)([^<]+)(</string>)",
            key
        ))
        .unwrap();
        let c = re
            .captures(&output)
            .ok_or("Info.plist field is missing; manual review required.")?;
        let old = c.get(2).unwrap().as_str().trim();
        if old.contains("$(") {
            return Err(
                "Info.plist is controlled by Xcode build settings; manual review required.".into(),
            );
        }
        if (key == "CFBundleShortVersionString" && !valid_version(old))
            || (key == "CFBundleVersion" && !numeric(old))
        {
            return Err("Info.plist has a non-static version field; manual review required.".into());
        }
        changed(
            format!("{}: {}", key, old),
            format!("{}: {}", key, new_value),
            &mut diffs,
        );
        output = re
            .replace_all(&output, format!("${{1}}{}${{3}}", new_value))
            .to_string();
    }
    Ok((output, diffs))
}
fn rewrite_desktop_json(
    input: &str,
    version: &str,
    build: &str,
    platform: &str,
    supports_build_number: bool,
) -> Result<(String, Vec<LineChange>), String> {
    let mut output = input.to_string();
    let mut diffs = vec![];
    if !version.is_empty() {
        let re = Regex::new(r#"(?m)^(\s*"version"\s*:\s*")([^"]+)(".*)$"#).unwrap();
        let values: Vec<_> = re
            .captures_iter(&output)
            .filter_map(|capture| capture.get(2))
            .map(|value| value.as_str().to_string())
            .collect();
        if values.len() != 1 || !valid_version(&values[0]) {
            return Err(format!("{platform} version is missing, non-static, or ambiguous; manual review required."));
        }
        changed(
            format!("version: {}", values[0]),
            format!("version: {version}"),
            &mut diffs,
        );
        output = re.replace(&output, format!("${{1}}{version}${{3}}")).to_string();
    }
    if !build.is_empty() {
        if !supports_build_number {
            return Err("Tauri does not have a standard static build-number field; leave Build empty or update your release process manually.".into());
        }
        let quoted = Regex::new(r#"("buildNumber"\s*:\s*")(\d+)(")"#).unwrap();
        let numeric = Regex::new(r#"("buildNumber"\s*:\s*)(\d+)(\s*[,}])"#).unwrap();
        let quoted_values: Vec<_> = quoted.captures_iter(&output).collect();
        let numeric_values: Vec<_> = numeric.captures_iter(&output).collect();
        if quoted_values.len() + numeric_values.len() != 1 {
            return Err("Electron build.buildNumber is missing or ambiguous; manual review required.".into());
        }
        if let Some(capture) = quoted_values.first() {
            let old = capture.get(2).unwrap().as_str();
            changed(format!("buildNumber: {old}"), format!("buildNumber: {build}"), &mut diffs);
            output = quoted.replace(&output, format!("${{1}}{build}${{3}}")).to_string();
        } else if let Some(capture) = numeric_values.first() {
            let old = capture.get(2).unwrap().as_str();
            changed(format!("buildNumber: {old}"), format!("buildNumber: {build}"), &mut diffs);
            output = numeric.replace(&output, format!("${{1}}{build}${{3}}")).to_string();
        }
    }
    Ok((output, diffs))
}
fn valid_version(v: &str) -> bool {
    let p: Vec<_> = v.split('.').collect();
    p.len() == 3 && p.iter().all(|x| numeric(x))
}
fn numeric(v: &str) -> bool {
    !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};

    #[test]
    fn previews_applies_and_backs_up_flutter_version() {
        let root = env::temp_dir().join(format!("versionpilot-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("pubspec.yaml"), "name: demo\nversion: 1.4.2+38\n").unwrap();
        let preview = preview(&root, "1.5.0", "39").unwrap();
        assert_eq!(preview.changes.len(), 1);
        apply(&root, preview).unwrap();
        assert!(fs::read_to_string(root.join("pubspec.yaml"))
            .unwrap()
            .contains("1.5.0+39"));
        assert!(
            fs::read_to_string(root.join(".klenuabuild-backup/pubspec.yaml"))
                .unwrap()
                .contains("1.4.2+38")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn changes_a_single_static_gradle_variable_definition() {
        let input = "def VERSION_NAME = \"1.4.2\"\ndef VERSION_CODE = 38\nandroid { defaultConfig { } }\nversionName VERSION_NAME\nversionCode VERSION_CODE\n";
        let (out, changes) = rewrite_gradle(input, "1.5.0", "39").unwrap();
        assert_eq!(changes.len(), 2);
        assert!(out.contains("VERSION_NAME = \"1.5.0\""));
        assert!(out.contains("VERSION_CODE = 39"));
    }

    #[test]
    fn changes_static_electron_version_and_build_number() {
        let input = "{\n  \"version\": \"1.4.2\",\n  \"build\": { \"buildNumber\": 38 }\n}\n";
        let (out, changes) = rewrite_desktop_json(input, "1.5.0", "39", "Electron", true).unwrap();
        assert_eq!(changes.len(), 2);
        assert!(out.contains("\"version\": \"1.5.0\""));
        assert!(out.contains("\"buildNumber\": 39"));
    }
}
