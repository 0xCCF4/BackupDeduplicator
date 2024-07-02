use crate::hash::GeneralHash;
use crate::path::FilePath;
use crate::stages::build::output::HashTreeFileEntryType;
use serde::Serialize;

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
