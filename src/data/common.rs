use std::cell::{RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::SystemTime;
use anyhow::{anyhow, Result};
use log::{error, warn};
use crate::utils;

type ResolveNodeFn = fn(&HandleIdentifier) -> Result<Weak<RefCell<FileContainer>>>;
type PathInScopeFn = fn(&Path) -> bool;


#[derive(Debug, Hash, PartialEq, Clone)]
pub enum GeneralHash {
    SHA256([u8; 32]),
}

impl GeneralHash {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            GeneralHash::SHA256(data) => data,
        }
    }

    pub fn new_sha256() -> Self {
        GeneralHash::SHA256([0; 32])
    }
}

static NULL_HASH_SHA256: GeneralHash = GeneralHash::SHA256([0; 32]);

#[derive(Debug, Clone, PartialEq)]
pub struct HandleIdentifier {
    pub inode: u64,
    pub drive: u64,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum FileState {
    NotProcessed,
    Analyzed,
    Error,
}

#[derive(Debug, Clone)]
pub struct FileInformation {
    pub id: HandleIdentifier,
    pub path: std::path::PathBuf,
    pub modified: u64,
    pub content_hash: GeneralHash,
    pub content_size: u64,
    pub state: FileState,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum DirectoryState {
    NotProcessed, // the directory has not been processed yet
    Evaluating, // the directory is being processed
    Analyzed, // the directory has been fully processed
    Error, // an error occurred while processing the directory, will bubble up to the parent directory
}

#[derive(Debug, Clone)]
pub struct DirectoryInformation {
    pub id: HandleIdentifier,
    pub path: std::path::PathBuf,
    pub modified: u64,
    pub content_hash: GeneralHash,
    pub number_of_children: u64,
    pub children: Vec<Rc<RefCell<FileContainer>>>,
    pub state: DirectoryState,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum SymlinkState {
    NotProcessed, // the symlink has not been processed yet
    Analyzed, // the symlink has been fully processed, target is analyzed or follow symlink is disabled
    Skipped, // if a loop is detected, we skip the symlink - a loop occurs if the symlink points to a directory that is in the processing state
    Error, // an error occurred while processing the symlink, will bubble up to the parent directory
}

#[derive(Debug, Clone)]
pub enum SymlinkTarget {
    File(Weak<RefCell<FileContainer>>), // if the symlink points to a file
    Path(PathBuf), // if follow symlinks is disabled, or path is outside the analysis scope
}

#[derive(Debug, Clone)]
pub struct SymlinkInformation {
    pub id: HandleIdentifier,
    pub path: PathBuf,
    pub modified: u64,
    pub content_hash: GeneralHash, // equal to the target file's hash or if not following symlinks, the symlink's path hashed
    pub target: SymlinkTarget,
    pub state: SymlinkState,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum OtherState {
    Analyzed,
    Error,
}

#[derive(Debug, Clone)]
pub struct OtherInformation {
    pub id: HandleIdentifier,
    pub state: OtherState,
}

#[derive(Debug, Clone)]
pub enum File {
    File(FileInformation),
    Directory(DirectoryInformation),
    Symlink(SymlinkInformation),
    Other(OtherInformation), // for unsupported file types like block devices, character devices, etc.
}

#[derive(Debug, Clone)]
pub enum FileContainer {
    InMemory(File),
    OnDisk(HandleIdentifier),
    DoesNotExist, // e.g. if symlink points to a non-existing file
}

// ---- IMPLEMENTATION ----

impl File {
    pub fn id(&self) -> &HandleIdentifier {
        match self {
            File::File(info) => &info.id,
            File::Directory(info) => &info.id,
            File::Symlink(info) => &info.id,
            File::Other(id) => &id.id,
        }
    }

    pub fn has_finished_analyzing(&self) -> bool {
        match self {
            File::File(info) => matches!(info.state, FileState::Analyzed | FileState::Error),
            File::Directory(info) => matches!(info.state, DirectoryState::Analyzed | DirectoryState::Error),
            File::Symlink(info) => matches!(info.state, SymlinkState::Analyzed | SymlinkState::Error),
            File::Other(info) => matches!(info.state, OtherState::Analyzed | OtherState::Error),
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
}

impl FileContainer {
    pub fn id(&self) -> Option<&HandleIdentifier> {
        match self {
            FileContainer::InMemory(file) => Some(file.id()),
            FileContainer::OnDisk(id) => Some(id),
            FileContainer::DoesNotExist => None,
        }
    }

    pub fn has_finished_analyzing(&self) -> bool {
        match self {
            FileContainer::InMemory(file) => file.has_finished_analyzing(),
            FileContainer::OnDisk(_) => true, // files loaded out of memory are assumed to be fully processed
            FileContainer::DoesNotExist => true,
        }
    }
}

impl File {
    pub fn new(path: PathBuf, lookup_id: ResolveNodeFn) -> Result<Self> {
        let metadata = fs::metadata(&path)?;
        let handle = same_file::Handle::from_path(&path)?;

        let inode = handle.ino();
        let drive = handle.dev();
        drop(handle);

        let modified_result = metadata.modified()
            .map(|time| time.duration_since(SystemTime::UNIX_EPOCH)
                .or(Err(anyhow!("Unable to convert modified date to UNIX_EPOCH")))
                .map(|duration| duration.as_secs())
            ).unwrap_or_else(|err| {
            error!("Error while reading modified date {:?}: {:?}", path, err);
            Ok(0)
        });

        let modified;
        let mut error = false;

        match modified_result {
            Ok(time) => modified = time,
            Err(err) => {
                error!("Error while processing file {:?}: {:?}", path, err);
                error = true;
                modified = 0;
            }
        }

        if metadata.is_symlink() {
            let looked_up_target_node_result = lookup_id(&HandleIdentifier {
                inode,
                drive,
            });
            match looked_up_target_node_result {
                Ok(weak) => {
                    Ok(Self::Symlink(SymlinkInformation {
                        id: HandleIdentifier {
                            inode,
                            drive,
                        },
                        content_hash: GeneralHash::new_sha256(),
                        path,
                        modified,
                        target: weak,
                        state: if error {SymlinkState::Error } else {SymlinkState::NotProcessed},
                    }))
                },
                Err(err) => {
                    error!("Error while resolving target of symlink {:?}: {:?}", path, err);
                    warn!("This indicates an error in the lookup target function, the symlink will be marked as error");

                    Ok(Self::Other(OtherInformation {
                        id: HandleIdentifier {
                            inode,
                            drive,
                        },
                        state: OtherState::Error,
                    }))
                }
            }
        } else if metadata.is_dir() {
            Ok(Self::Directory(DirectoryInformation {
                id: HandleIdentifier {
                    inode,
                    drive,
                },
                path,
                modified,
                number_of_children: 0,
                content_hash: GeneralHash::new_sha256(),
                children: Vec::new(),
                state: DirectoryState::NotProcessed,
            }))
        } else if metadata.is_file() {
            Ok(Self::File(FileInformation {
                id: HandleIdentifier {
                    inode,
                    drive,
                },
                path,
                modified,
                content_hash: GeneralHash::new_sha256(),
                content_size: 0,
                state: FileState::NotProcessed,
            }))
        } else {
            Ok(Self::Other(OtherInformation {
                id: HandleIdentifier {
                    inode,
                    drive,
                },
                state: OtherState::Analyzed,
            }))
        }
    }
}

impl FileInformation {
    pub fn analyze(&mut self) {
        if self.state != FileState::NotProcessed {
            return;
        }

        match fs::File::open(&self.path) {
            Ok(file) => {
                let mut reader = std::io::BufReader::new(file);
                match utils::hash_file(&mut reader, &mut self.content_hash) {
                    Ok(size) => {
                        self.content_size = size;
                        self.state = FileState::Analyzed;
                    }
                    Err(err) => {
                        error!("Error while hashing file {:?}: {:?}", self.path, err);
                        self.state = FileState::Error;
                    }
                }
            }
            Err(err) => {
                error!("Error while opening file {:?}: {:?}", self.path, err);
                self.state = FileState::Error;
            }
        }
    }
}

impl DirectoryInformation {
    pub fn analyze(&mut self) {
        if self.state == DirectoryState::Evaluating {
            return;
        }

        // check if all children are analyzed

        for child in &self.children {
            let child = child.borrow(); // todo check
            if !child.has_finished_analyzing() {
                return;
            }
        }

        // if all children are analyzed, calculate hash

        let hash_result = utils::hash_directory(self.children.iter(), &mut self.content_hash);
        match hash_result {
            Ok(size) => {
                self.number_of_children = size;
                self.state = DirectoryState::Analyzed;
            }
            Err(err) => {
                error!("Error while hashing directory {:?}: {:?}", self.path, err);
                self.state = DirectoryState::Error;
            }
        }
    }
}

impl SymlinkInformation {
    pub fn analyze(&mut self, follow_symlinks: bool, load_id: ResolveNodeFn) {
        if self.state != SymlinkState::NotProcessed {
            return;
        }

        if !follow_symlinks {
            self.content_hash = GeneralHash::SHA256(utils::hash_path(&self.path));
            self.state = SymlinkState::Analyzed;
            return;
        }
    }
}