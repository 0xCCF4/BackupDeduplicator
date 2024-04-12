use serde::{Deserialize, Serialize};

/// The type of archive.
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum ArchiveType {
    #[cfg(feature = "tar")]
    Tar,
}
