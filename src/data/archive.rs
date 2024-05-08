use std::io::Read;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// The type of archive.
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum ArchiveType {
    #[cfg(feature = "archive-tar")]
    Tar,
}

/// An entry in an archive.
/// 
/// # Fields
/// * `path` - The path of the entry.
/// * `size` - The size of the entry.
/// * `stream` - The stream of the entry.
pub struct ArchiveEntry {
    pub path: PathBuf,
    pub size: u64,
    pub stream: Box<dyn Read>,
}

#[cfg(feature = "archive-tar")]
pub mod tar;
