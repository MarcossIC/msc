// Git status integration
use std::path::Path;
use std::collections::HashMap;
use git2::{Repository, Status};

#[derive(Debug, Clone, PartialEq)]
pub enum GitStatus {
    Untracked,    // New file (green)
    Modified,     // Modified file (green)
    Added,        // Staged file (green)
    Deleted,      // Deleted file (red strikethrough)
    Renamed,      // Renamed file (green)
    Clean,        // No changes
}

pub fn load_git_status(dir_path: &Path) -> Option<HashMap<String, GitStatus>> {
    let repo = Repository::discover(dir_path).ok()?;
    let mut status_map = HashMap::new();

    let statuses = repo.statuses(None).ok()?;

    for entry in statuses.iter() {
        let path = entry.path()?;
        let flags = entry.status();

        let git_status = if flags.contains(Status::WT_DELETED) || flags.contains(Status::INDEX_DELETED) {
            GitStatus::Deleted
        } else if flags.contains(Status::WT_NEW) || flags.contains(Status::INDEX_NEW) {
            if flags.contains(Status::INDEX_NEW) {
                GitStatus::Added
            } else {
                GitStatus::Untracked
            }
        } else if flags.contains(Status::WT_MODIFIED) || flags.contains(Status::INDEX_MODIFIED) {
            GitStatus::Modified
        } else if flags.contains(Status::WT_RENAMED) || flags.contains(Status::INDEX_RENAMED) {
            GitStatus::Renamed
        } else {
            GitStatus::Clean
        };

        status_map.insert(path.to_string(), git_status);
    }

    Some(status_map)
}

pub fn get_git_status_for_file(
    git_status_map: &Option<HashMap<String, GitStatus>>,
    file_path: &Path,
    base_path: &Path
) -> GitStatus {
    if let Some(status_map) = git_status_map {
        if let Ok(relative_path) = file_path.strip_prefix(base_path) {
            let path_str = relative_path.to_string_lossy().replace('\\', "/");
            return status_map.get(&path_str).cloned().unwrap_or(GitStatus::Clean);
        }
    }
    GitStatus::Clean
}
