use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// The type of archive.
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum ArchiveType {
    #[cfg(feature = "archive-tar")]
    Tar,
}

pub struct ArchiveEntry {
    pub path: PathBuf,
    pub size: u64,
}

#[cfg(feature = "archive-tar")]
pub mod tar;
