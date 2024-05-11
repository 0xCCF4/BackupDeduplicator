use std::io::Read;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// The type of archive.
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum ArchiveType {
    #[cfg(feature = "archive-tar")]
    Tar,
}

impl ArchiveType {
    pub fn open<R: Read + 'static>(&self, stream: R) -> Result<Box<dyn Iterator<Item=Result<ArchiveEntry>>>> {
        match self {
            #[cfg(feature = "archive-tar")]
            ArchiveType::Tar => Ok(Box::new(tar::TarArchiveIterator::new(stream)?) as Box<dyn Iterator<Item = Result<ArchiveEntry, anyhow::Error>>>),
        }
    }
    
    pub fn from_extension(extension: &str) -> Option<ArchiveType> {
        match extension {
            #[cfg(feature = "archive-tar")]
            "tar" => Some(ArchiveType::Tar),
            _ => None,
        }
    }
}

/// An entry in an archive.
/// 
/// # Fields
/// * `path` - The path of the entry.
/// * `size` - The size of the entry.
/// * `modified` - The last modified time of the entry.
/// * `stream` - The stream of the entry.
pub struct ArchiveEntry {
    pub path: PathBuf,
    pub size: u64,
    pub modified: u64,
    pub stream: Box<dyn Read>,
}

#[cfg(feature = "archive-tar")]
pub mod tar;
