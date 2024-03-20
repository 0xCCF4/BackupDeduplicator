use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::data::{FilePath, GeneralHash};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisFile {
    File(FileInformation),
    Directory(DirectoryInformation),
    Symlink(SymlinkInformation),
    Other(OtherInformation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
    pub children: Vec<Arc<AnalysisFile>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherInformation {
    pub path: FilePath,
}
