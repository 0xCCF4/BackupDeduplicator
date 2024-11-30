use crate::hash::GeneralHash;
use crate::path::FilePath;
use serde::{Deserialize, Serialize};

/// Deduplication actions file version.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum DeduplicationActionVersion {
    /// Version 1 of the file format.
    V1,
}

/// Deduplication actions file.
///
/// # Fields
/// * `version` - The version of the file format.
/// * `actions` - The deduplication actions.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeduplicationActions {
    /// The version of the file format.
    pub version: DeduplicationActionVersion,
    /// The deduplication actions.
    pub actions: Vec<DeduplicationAction>,
}

/// An actions to be taken to deduplicate files.
/// It can be assumed that the remaining duplicates are not removed.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DeduplicationAction {
    /// Remove a file.
    RemoveFile {
        /// The path of the file to remove.
        path: FilePath,
        /// A list of remaining other duplicates.
        /// When reducing disk space by hard-linking files, the file can
        /// be preserved but hard-linked to this file instead of deleting it.
        remaining_duplicates: Vec<FilePath>,
        /// Hash of the file to remove.
        hash: GeneralHash,
        /// Size of the file to remove.
        size: u64,
    },
    /// Remove a directory.
    RemoveDirectory {
        /// The path of the directory to remove.
        path: FilePath,
        /// A list of remaining other duplicates.
        /// When reducing disk space by symlinking directories, the directory can
        /// be preserved but symlinked to this directory instead of deleting it.
        remaining_duplicates: Vec<FilePath>,
        /// Hash of the directory to remove.
        hash: GeneralHash,
        /// Number of children
        children: u64,
    },
}
