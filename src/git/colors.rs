// Git status color formatting
use colored::ColoredString;
use colored::Colorize;
use crate::git::status::GitStatus;

pub fn apply_git_colors(text: String, git_status: &GitStatus, is_dir: bool, is_dimmed: bool) -> ColoredString {
    match git_status {
        GitStatus::Deleted => {
            // Red strikethrough for deleted files
            text.red().strikethrough()
        }
        GitStatus::Untracked | GitStatus::Modified | GitStatus::Added | GitStatus::Renamed => {
            // Light green for new/modified files
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
