use serde::Serialize;
use std::{path::Path, process::Command};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub branch: Option<String>,
    pub status: String,
}

pub fn inspect(root: &Path) -> Option<GitInfo> {
    let branch = Command::new("git")
        .args(["-C", root.to_str()?, "branch", "--show-current"])
        .output()
        .ok()?;
    if !branch.status.success() {
        return None;
    }
    let status = Command::new("git")
        .args(["-C", root.to_str()?, "status", "--porcelain"])
        .output()
        .ok()?;
    Some(GitInfo {
        branch: Some(String::from_utf8_lossy(&branch.stdout).trim().to_string()),
        status: if status.stdout.is_empty() {
            "Clean".into()
        } else {
            "Uncommitted changes".into()
        },
    })
}
