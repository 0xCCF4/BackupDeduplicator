use std::path::{PathBuf};
use serde::{Deserialize, Serialize};
use crate::data::{FilePath, GeneralHash, NULL_HASH_SHA256};

// type ResolveNodeFn = fn(&HandleIdentifier) -> Result<Rc<RefCell<FileContainer>>>;
// type PathInScopeFn = fn(&Path) -> bool;



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInformation {
    pub path: FilePath,
    pub modified: u64,
    pub content_hash: GeneralHash,
    pub content_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryInformation {
    pub path: FilePath,
    pub modified: u64,
    pub content_hash: GeneralHash,
    pub number_of_children: u64,
    pub children: Vec<File>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkInformation {
    pub path: FilePath,
    pub modified: u64,
    pub content_hash: GeneralHash, // equal to the target file's hash or if not following symlinks, the symlink's path hashed
    pub target: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherInformation {
    pub path: FilePath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubInformation {
    pub path: FilePath,
    pub content_hash: GeneralHash,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum File {
    File(FileInformation),
    Directory(DirectoryInformation),
    Symlink(SymlinkInformation),
    Other(OtherInformation), // for unsupported file types like block devices, character devices, etc., or files without permission
    Stub(StubInformation), // for files that are already analyzed
}

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

    pub fn get_content_hash(&self) -> &GeneralHash {
        match self {
            File::File(info) => &info.content_hash,
            File::Directory(info) => &info.content_hash,
            File::Symlink(info) => &info.content_hash,
            File::Other(_) => &NULL_HASH_SHA256,
            File::Stub(info) => &info.content_hash,
        }
    }
    
    pub fn get_path(&self) -> &FilePath {
        match self {
            File::File(info) => &info.path,
            File::Directory(info) => &info.path,
            File::Symlink(info) => &info.path,
            File::Other(info) => &info.path,
            File::Stub(info) => &info.path,
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

