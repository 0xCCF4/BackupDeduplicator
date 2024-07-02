use crate::hash::GeneralHash;
use crate::path::FilePath;
use serde::{Deserialize, Serialize};
use std::sync::Weak;
use std::sync::{Arc, Mutex};

/// The result of the analysis worker.
#[derive(Debug, Serialize, Deserialize)]
pub enum AnalysisFile {
    File(AnalysisFileInformation),
    Directory(AnalysisDirectoryInformation),
    Symlink(AnalysisSymlinkInformation),
    Other(AnalysisOtherInformation),
}

impl AnalysisFile {
    /// Get the parent of the file.
    ///
    /// # Returns
    /// The parent of the file. None if the file has no parent.
    pub fn parent(&self) -> &Mutex<Option<Weak<AnalysisFile>>> {
        match self {
            AnalysisFile::File(info) => &info.parent,
            AnalysisFile::Directory(info) => &info.parent,
            AnalysisFile::Symlink(info) => &info.parent,
            AnalysisFile::Other(info) => &info.parent,
        }
    }

    /// Get the path of the file.
    ///
    /// # Returns
    /// The path of the file.
    pub fn path(&self) -> &FilePath {
        match self {
            AnalysisFile::File(info) => &info.path,
            AnalysisFile::Directory(info) => &info.path,
            AnalysisFile::Symlink(info) => &info.path,
            AnalysisFile::Other(info) => &info.path,
        }
    }
}

/// File information part of [AnalysisFile].
///
/// # Fields
/// * `path` - The path of the file.
/// * `content_hash` - The hash of the file content.
/// * `parent` - The parent of the file.
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisFileInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
    pub parent: Mutex<Option<Weak<AnalysisFile>>>,
}

/// Directory information part of [AnalysisFile].
///
/// # Fields
/// * `path` - The path of the directory.
/// * `content_hash` - The hash of the directory content.
/// * `children` - The children of the directory.
/// * `parent` - The parent of the directory.
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisDirectoryInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
    pub children: Mutex<Vec<Arc<AnalysisFile>>>,
    pub parent: Mutex<Option<Weak<AnalysisFile>>>,
}

/// Symlink information part of [AnalysisFile].
///
/// # Fields
/// * `path` - The path of the symlink.
/// * `content_hash` - The hash of the symlink content.
/// * `parent` - The parent of the symlink.
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisSymlinkInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
    pub parent: Mutex<Option<Weak<AnalysisFile>>>,
}

/// Other information part of [AnalysisFile].
///
/// # Fields
/// * `path` - The path of the file.
/// * `parent` - The parent of the file.
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisOtherInformation {
    pub path: FilePath,
    pub parent: Mutex<Option<Weak<AnalysisFile>>>,
}
