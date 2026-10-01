use crate::backup;
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

pub fn changed(before: String, after: String, d: &mut Vec<LineChange>) {
    if before != after {
        d.push(LineChange { before, after })
    }
}

/// Backs up and writes every changed file in a preview — shared by every
/// command that produces a `Preview` (build, rename): the write side never
/// differs by what generated the diff.
pub fn apply(root: &Path, preview: Preview) -> Result<(), String> {
    for file in preview.changes {
        backup::save(root, &file.absolute, &file.original)?;
        fs::write(&file.absolute, file.updated)
            .map_err(|e| format!("Could not write {}: {}", file.file, e))?;
    }
    Ok(())
}
