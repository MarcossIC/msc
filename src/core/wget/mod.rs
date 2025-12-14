// Wget module - Web page downloading functionality

pub mod wget_cookies;
pub mod wget_manager;
pub mod wget_utils;
pub mod wgetpostprocessing;

// Re-export commonly used items
pub use wget_cookies::{
    create_cookie_file, debug_database_info, extract_cookies_from_db, find_browser_cookie_db,
    format_cookies, resolve_cookie_path, Cookie,
};
pub use wget_manager::WgetManager;
pub use wget_utils::{
    calculate_local_path_for_url, calculate_possible_local_paths, download_resource,
    extract_filename_from_url, is_local_path, is_placeholder_image,
};
pub use wgetpostprocessing::process_html_file_complete;
