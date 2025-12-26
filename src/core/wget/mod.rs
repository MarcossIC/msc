// Wget module - Web page downloading functionality

pub mod cdp_cookies;
pub mod chrome_decrypt;
pub mod chrome_launcher;
pub mod chrome_manager;
pub mod cookie_formats;
pub mod dpapi;
pub mod wget_cookies;
pub mod wget_manager;
pub mod wget_utils;
pub mod wgetpostprocessing;

// Re-export commonly used items
pub use cdp_cookies::{
    extract_cookies_cdp, get_cookies_for_domain, is_cdp_available, print_cdp_instructions,
};
pub use chrome_decrypt::ChromeDecryptor;
pub use chrome_launcher::ChromeInstance;
pub use chrome_manager::ChromeManager;
pub use cookie_formats::{
    chrome_time_to_unix, format_cookies as format_cookies_util, format_json, format_netscape,
    format_wget,
};
pub use dpapi::decrypt_dpapi;
pub use wget_cookies::{
    create_cookie_file, debug_database_info, extract_cookies_from_db, extract_cookies_with_cdp,
    find_browser_cookie_db, format_cookies, resolve_cookie_path, Cookie,
};
pub use wget_manager::WgetManager;
pub use wget_utils::{
    calculate_local_path_for_url, calculate_possible_local_paths, download_resource,
    extract_filename_from_url, is_local_path, is_placeholder_image,
};
pub use wgetpostprocessing::process_html_file_complete;
