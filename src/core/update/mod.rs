mod install_detector;
mod manager;
mod platform_installer;
mod release_info;

pub use install_detector::{detect_install_method, InstallMethod};
pub use manager::UpdateManager;
pub use release_info::ReleaseInfo;
