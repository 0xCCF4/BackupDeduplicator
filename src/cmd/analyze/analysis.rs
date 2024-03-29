use std::sync::Weak;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use crate::data::{FilePath, GeneralHash, SaveFileEntryType};

#[derive(Debug, Serialize, Deserialize)]
pub enum AnalysisFile {
    File(FileInformation),
    Directory(DirectoryInformation),
    Symlink(SymlinkInformation),
    Other(OtherInformation),
}

impl AnalysisFile {
    pub fn parent(&self) -> &Mutex<Option<Weak<AnalysisFile>>> {
        match self {
            AnalysisFile::File(info) => &info.parent,
            AnalysisFile::Directory(info) => &info.parent,
            AnalysisFile::Symlink(info) => &info.parent,
            AnalysisFile::Other(info) => &info.parent,
        }
    }

    pub fn path(&self) -> &FilePath {
        match self {
            AnalysisFile::File(info) => &info.path,
            AnalysisFile::Directory(info) => &info.path,
            AnalysisFile::Symlink(info) => &info.path,
            AnalysisFile::Other(info) => &info.path,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
    pub parent: Mutex<Option<Weak<AnalysisFile>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
    pub children: Mutex<Vec<Arc<AnalysisFile>>>,
    pub parent: Mutex<Option<Weak<AnalysisFile>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SymlinkInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
    pub parent: Mutex<Option<Weak<AnalysisFile>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OtherInformation {
    pub path: FilePath,
    pub parent: Mutex<Option<Weak<AnalysisFile>>>,
}


#[derive(Debug, Serialize)]
pub struct ResultEntryRef<'a, 'b, 'c> {
    pub ftype: &'a SaveFileEntryType,
    pub size: u64,
    pub hash: &'b GeneralHash,
    pub conflicting: Vec<&'c FilePath>,
}
