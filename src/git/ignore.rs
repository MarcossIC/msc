// Gitignore integration
use std::path::Path;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

pub fn load_gitignore(dir_path: &Path) -> Option<Gitignore> {
    let mut builder = GitignoreBuilder::new(dir_path);

    // Try to add .gitignore file if it exists
    let gitignore_path = dir_path.join(".gitignore");
    if gitignore_path.exists()
        && builder.add(&gitignore_path).is_some() {
            return None;
        }

    // Try to find parent directories with .gitignore
    let mut current = dir_path.parent();
    while let Some(parent) = current {
        let parent_gitignore = parent.join(".gitignore");
        if parent_gitignore.exists() {
            let _ = builder.add(&parent_gitignore);
            break;
        }
        current = parent.parent();
    }

    builder.build().ok()
}

pub fn is_gitignored(gitignore: &Option<Gitignore>, path: &Path, is_dir: bool) -> bool {
    if let Some(gi) = gitignore {
        matches!(gi.matched(path, is_dir), ignore::Match::Ignore(_))
    } else {
        false
    }
}
