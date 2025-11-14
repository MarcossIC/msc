// Platform-specific filesystem operations

#[cfg(windows)]
pub fn is_hidden(entry: &std::fs::DirEntry) -> bool {
    const FILE_ATTRIBUTE_HIDDEN: u32 = 2;

    if let Ok(metadata) = entry.metadata() {
        use std::os::windows::fs::MetadataExt;
        let attributes = metadata.file_attributes();
        (attributes & FILE_ATTRIBUTE_HIDDEN) != 0
    } else {
        false
    }
}

#[cfg(not(windows))]
pub fn is_hidden(_entry: &std::fs::DirEntry) -> bool {
    false
}
