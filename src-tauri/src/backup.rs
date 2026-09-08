use std::{
    fs,
    path::{Path, PathBuf},
};

fn directory(root: &Path) -> PathBuf {
    root.join(".klenuabuild-backup")
}

fn legacy_directory(root: &Path) -> PathBuf {
    root.join(".versionpilot-backup")
}

pub fn latest_files(root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let current = directory(root);
    let backup = if current.exists() { current } else { legacy_directory(root) };
    if !backup.exists() {
        return Err("No klenua build backup is available for this project.".into());
    }
    let files = visit(&backup)?;
    if files.is_empty() {
        return Err("No klenua build backup is available for this project.".into());
    }
    files.into_iter().map(|file| {
        let relative = file.strip_prefix(&backup).map_err(|error| error.to_string())?
            .to_string_lossy().replace('\\', "/");
        Ok((relative, file))
    }).collect()
}
pub fn save(root: &Path, file: &Path, original: &str) -> Result<(), String> {
    let relative = file
        .strip_prefix(root)
        .map_err(|_| "File is not inside project")?;
    let destination = directory(root).join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(destination, original).map_err(|e| e.to_string())
}
pub fn restore(root: &Path) -> Result<String, String> {
    let current = directory(root);
    let backup = if current.exists() { current } else { legacy_directory(root) };
    if !backup.exists() {
        return Err("No klenua build backup is available for this project.".into());
    }
    let files = visit(&backup)?;
    if files.is_empty() {
        return Err("No klenua build backup is available for this project.".into());
    }
    let mut count = 0;
    for source in files {
        let relative = source.strip_prefix(&backup).map_err(|e| e.to_string())?;
        let destination = root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(&source, &destination).map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(format!(
        "Restored {} file{} from the latest klenua build backup.",
        count,
        if count == 1 { "" } else { "s" }
    ))
}
fn visit(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
            continue;
        }
        let p = entry.path();
        if p.is_dir() {
            result.extend(visit(&p)?);
        } else {
            result.push(p);
        }
    }
    Ok(result)
}
