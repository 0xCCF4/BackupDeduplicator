use crate::hash::GeneralHash;
use crate::path::FilePath;
use crate::stages::build::output::HashTreeFileEntryType;
use serde::{Deserialize, Serialize};

/// The result of the analysis worker. A duplicate set entry.
///
/// # Fields
/// * `ftype` - The type of the file.
/// * `size` - The size of the file.
/// * `hash` - The hash of the file content.
/// * `conflicting` - The conflicting files.
#[derive(Debug, Serialize)]
pub struct DupSetEntryRef<'a, 'b, 'c> {
    /// The type of the file.
    pub ftype: &'a HashTreeFileEntryType,
    /// The size of the file.
    pub size: u64,
    /// The hash of the file content.
    pub hash: &'b GeneralHash,
    /// The conflicting files.
    pub conflicting: Vec<&'c FilePath>,
}

/// The result of the analysis worker. A duplicate set entry.
///
/// # Fields
/// * `ftype` - The type of the file.
/// * `size` - The size of the file.
/// * `hash` - The hash of the file content.
/// * `conflicting` - The conflicting files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DupSetEntry {
    /// The type of the file.
    pub ftype: HashTreeFileEntryType,
    /// The size of the file.
    pub size: u64,
    /// The hash of the file content.
    pub hash: GeneralHash,
    /// The conflicting files.
    pub conflicting: Vec<FilePath>,
}

impl From<&DupSetEntryRef<'_, '_, '_>> for DupSetEntry {
    fn from(entry: &DupSetEntryRef) -> Self {
        DupSetEntry {
            ftype: *entry.ftype,
            size: entry.size,
            hash: entry.hash.clone(),
            conflicting: entry
                .conflicting
                .clone()
                .into_iter()
                .cloned()
                .collect::<Vec<FilePath>>(),
        }
    }
}

/// Deduplication set file version.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum DupSetFileVersion {
    /// Version 1 of the file format.
    V1,
}

/// Deduplication set file.
///
/// # Fields
/// * `version` - The version of the file format.
/// * `entries` - The deduplication set entries.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DupSetFile {
    /// The version of the file format.
    pub version: DupSetFileVersion,
    /// The deduplication set entries.
    pub entries: Vec<DupSetEntry>,
}

/// Deduplication set file. (Reference version)
///
/// # Fields
/// * `version` - The version of the file format.
/// * `entries` - The deduplication set entries.
#[derive(Debug, Serialize)]
pub struct DupSetFileRef<'a, 'b, 'c> {
    /// The version of the file format.
    pub version: DupSetFileVersion,
    /// The deduplication set entries.
    pub entries: Vec<DupSetEntryRef<'a, 'b, 'c>>,
}

impl From<&DupSetFileRef<'_, '_, '_>> for DupSetFile {
    fn from(value: &DupSetFileRef<'_, '_, '_>) -> Self {
        DupSetFile {
            version: value.version,
            entries: value
                .entries
                .iter()
                .map(DupSetEntry::from)
                .collect::<Vec<DupSetEntry>>(),
        }
    }
}
