use serde::{Deserialize, Serialize};
use crate::hash::GeneralHash;

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
    pub version: DeduplicationActionVersion,
    pub actions: Vec<DeduplicationAction>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DeduplicationAction {
    /// Remove a file.
    RemoveFile {
        /// The path of the file to remove.
        path: String,
        /// A list of remaining other duplicates.
        /// When reducing disk space by hard-linking files, the file can
        /// be preserved but hard-linked to this file instead of deleting it.
        remaining_duplicates: Vec<String>,
        /// Hash of the file to remove.
        hash: GeneralHash,
        /// Size of the file to remove.
        size: u64,
    },
    RemoveDirectory {
        /// The path of the directory to remove.
        path: String,
        /// A list of remaining other duplicates.
        /// When reducing disk space by symlinking directories, the directory can
        /// be preserved but symlinked to this directory instead of deleting it.
        remaining_duplicates: Vec<String>,
        /// Hash of the directory to remove.
        hash: GeneralHash,
        /// Number of children
        children: u64,
    },
}