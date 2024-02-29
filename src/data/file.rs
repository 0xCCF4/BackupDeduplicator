use std::path::{PathBuf};
use std::sync::{Arc, Mutex};
use serde::{Serialize};
use crate::data::{FilePath, GeneralHash, NULL_HASH_SHA256};

// type ResolveNodeFn = fn(&HandleIdentifier) -> Result<Rc<RefCell<FileContainer>>>;
// type PathInScopeFn = fn(&Path) -> bool;




#[derive(Debug, Clone, PartialEq, Copy, Serialize)]
pub enum FileState {
    NotProcessed,
    Analyzed,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInformation {
    pub path: FilePath,
    pub modified: u64,
    pub content_hash: GeneralHash,
    pub content_size: u64,
    pub state: FileState,
}

#[derive(Debug, Clone, PartialEq, Copy, Serialize)]
pub enum DirectoryState {
    NotProcessed, // the directory has not been processed yet
    Evaluating, // the directory is being processed
    Analyzed, // the directory has been fully processed
    Error, // an error occurred while processing the directory, will bubble up to the parent directory
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectoryInformation {
    pub path: FilePath,
    pub modified: u64,
    pub content_hash: GeneralHash,
    pub number_of_children: u64,
    pub children: Vec<SharedFile>,
    pub state: DirectoryState,
}

#[derive(Debug, Clone, PartialEq, Copy, Serialize)]
pub enum SymlinkState {
    NotProcessed, // the symlink has not been processed yet
    Analyzed, // the symlink has been fully processed, target is analyzed or follow symlink is disabled
    Skipped, // if a loop is detected, we skip the symlink - a loop occurs if the symlink points to a directory that is in the processing state
    Error, // an error occurred while processing the symlink, will bubble up to the parent directory
}

#[derive(Debug, Clone, Serialize)]
pub enum SymlinkTarget {
    // File(HandleIdentifier, Weak<RefCell<FileContainer>>), // if the symlink points to a file
    Path(PathBuf), // if follow symlinks is disabled, or path is outside the analysis scope
}

#[derive(Debug, Clone, Serialize)]
pub struct SymlinkInformation {
    pub path: FilePath,
    pub modified: u64,
    pub content_hash: GeneralHash, // equal to the target file's hash or if not following symlinks, the symlink's path hashed
    pub target: SymlinkTarget,
    pub state: SymlinkState,
}

#[derive(Debug, Clone, Serialize)]
pub struct OtherInformation {
    pub path: PathBuf,
}


#[derive(Debug, Clone, Serialize)]
pub enum File {
    File(FileInformation),
    Directory(DirectoryInformation),
    Symlink(SymlinkInformation),
    Other(OtherInformation), // for unsupported file types like block devices, character devices, etc., or files without permission
}

pub type SharedFile = Arc<Mutex<File>>;

// ---- IMPLEMENTATION ----

impl File {
    /*
    pub fn id(&self) -> &HandleIdentifier {
        match self {
            File::File(info) => &info.id,
            File::Directory(info) => &info.id,
            File::Symlink(info) => &info.id,
            File::Other(_) => &NULL_HANDLE,
        }
    }*/

    pub fn has_finished_analyzing(&self) -> bool {
        match self {
            File::File(info) => matches!(info.state, FileState::Analyzed | FileState::Error),
            File::Directory(info) => matches!(info.state, DirectoryState::Analyzed | DirectoryState::Error),
            File::Symlink(info) => matches!(info.state, SymlinkState::Analyzed | SymlinkState::Error),
            File::Other(_) => true,
        }
    }

    pub fn get_content_hash(&self) -> &GeneralHash {
        match self {
            File::File(info) => &info.content_hash,
            File::Directory(info) => &info.content_hash,
            File::Symlink(info) => &info.content_hash,
            File::Other(_) => &NULL_HASH_SHA256,
        }
    }

    pub fn is_directory(&self) -> bool {
        match self {
            File::Directory(_) => true,
            _ => false,
        }
    }

    pub fn is_symlink(&self) -> bool {
        match self {
            File::Symlink(_) => true,
            _ => false,
        }
    }

    pub fn is_file(&self) -> bool {
        match self {
            File::File(_) => true,
            _ => false,
        }
    }

    pub fn is_other(&self) -> bool {
        match self {
            File::Other(_) => true,
            _ => false,
        }
    }
}

/*
impl FileContainer {
    #[deprecated]
    pub fn id(&self) -> HandleIdentifier {
        match self {
            FileContainer::InMemory(file) => file.borrow().id().clone(),
            // FileContainer::OnDisk(id) => id.clone(),
        }
    }

    pub fn has_finished_analyzing(&self) -> bool {
        match self {
            FileContainer::InMemory(file) => file.borrow().has_finished_analyzing(),
            // FileContainer::OnDisk(_) => true, // files loaded out of memory are assumed to be fully processed
        }
    }

    pub fn has_errored(&self) -> bool {
        match self {
            FileContainer::InMemory(file) => match file.borrow().deref() {
                File::File(info) => matches!(info.state, FileState::Error),
                File::Directory(info) => matches!(info.state, DirectoryState::Error),
                File::Symlink(info) => matches!(info.state, SymlinkState::Error),
                File::Other(_) => false,
            },
            // FileContainer::OnDisk(_) => false, // files loaded out of memory are assumed to be fully processed
        }
    }
}

 */
