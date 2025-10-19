use crate::git::status::GitStatus;
use colored::ColoredString;
use colored::Colorize;

pub fn apply_git_colors(
    text: String,
    git_status: &GitStatus,
    is_dir: bool,
    is_dimmed: bool,
) -> ColoredString {
    match git_status {
        GitStatus::Deleted => {
            text.red().strikethrough()
        }
        GitStatus::Untracked | GitStatus::Modified | GitStatus::Added | GitStatus::Renamed => {
            text.bright_green()
        }
        GitStatus::Clean => {
            // Normal colors based on file type and dimmed status
            if is_dir {
                if is_dimmed {
                    text.blue().dimmed()
                } else {
                    text.blue().bold()
                }
            } else if is_dimmed {
                text.bright_black()
            } else {
                text.white()
            }
        }
    }
}
