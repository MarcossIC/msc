mod cdn_extraction;
mod path_conversion;
mod quirk_fixes;
mod resource_discovery;
mod url_replacement;

pub use cdn_extraction::extract_cdn_resources;
pub use path_conversion::convert_absolute_paths;
pub use quirk_fixes::apply_quirk_fixes;
pub use resource_discovery::discover_and_download_resources;
pub use url_replacement::apply_replacements;
