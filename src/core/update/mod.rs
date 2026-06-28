mod install_detector;
mod manager;
mod platform_installer;
mod release_info;

pub use install_detector::{classify_windows_install_path, detect_install_method, update_target, InstallMethod, UpdateTarget};
pub use manager::UpdateManager;
pub use release_info::ReleaseInfo;
