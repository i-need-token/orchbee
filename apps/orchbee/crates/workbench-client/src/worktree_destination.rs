//! One configured checkout destination policy for the CLI and native Worktree form.
use orchbee_workbench_protocol::{WorkbenchDirectoryPath, WorkbenchDirectoryPathError};
use std::path::Path;

/// Joins a configured root, repository name and bounded branch slug.
///
/// # Errors
/// Returns a path error when the resulting UTF-8 absolute path exceeds the protocol bound.
pub fn destination(
    root: &WorkbenchDirectoryPath,
    repository: &WorkbenchDirectoryPath,
    branch: &str,
) -> Result<WorkbenchDirectoryPath, WorkbenchDirectoryPathError> {
    let repository = Path::new(repository.as_str())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repository");
    let path = Path::new(root.as_str()).join(repository).join(slug(branch));
    WorkbenchDirectoryPath::parse(path.to_str().ok_or(WorkbenchDirectoryPathError)?)
}
fn slug(branch: &str) -> String {
    let mut result = String::new();
    for character in branch.chars() {
        if character.is_ascii_alphanumeric() {
            result.push(character.to_ascii_lowercase());
        } else if !result.is_empty() && !result.ends_with('-') {
            result.push('-');
        }
    }
    let length = result.trim_end_matches('-').len();
    result.truncate(length);
    if result.is_empty() {
        result.push_str("worktree");
    }
    result
}
#[cfg(test)]
mod tests;
