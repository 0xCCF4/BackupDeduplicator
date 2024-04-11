use serde::{Serialize};
use crate::hash::GeneralHash;
use crate::path::FilePath;
use crate::stages::build::output::HashTreeFileEntryType;

/// The result of the analysis worker. A duplicate set entry.
/// 
/// # Fields
/// * `ftype` - The type of the file.
/// * `size` - The size of the file.
/// * `hash` - The hash of the file content.
/// * `conflicting` - The conflicting files.
#[derive(Debug, Serialize)]
pub struct DupSetEntryRef<'a, 'b, 'c> {
    pub ftype: &'a HashTreeFileEntryType,
    pub size: u64,
    pub hash: &'b GeneralHash,
    pub conflicting: Vec<&'c FilePath>,
}
