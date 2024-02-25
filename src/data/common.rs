use std::cell::{RefCell};
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::SystemTime;
use anyhow::{anyhow, Result};
use log::{debug, error, warn};
use serde::{Serialize, Serializer};
use crate::utils;

type ResolveNodeFn = fn(&HandleIdentifier) -> Result<Rc<RefCell<FileContainer>>>;
type PathInScopeFn = fn(&Path) -> bool;


#[derive(Debug, Hash, PartialEq, Clone)]
pub enum GeneralHash {
    SHA256([u8; 32]),
}

impl Serialize for GeneralHash {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> where S: Serializer {
        match self {
            GeneralHash::SHA256(data) => {
                // to hex string
                let mut hex = String::with_capacity(64);
                for byte in data {
                    hex.push_str(&format!("{:02x}", byte));
                }
                serializer.serialize_str(&hex)
            }
        }
    }
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HandleIdentifier {
    pub inode: u64,
    pub drive: u64,
}

static NULL_HANDLE: HandleIdentifier = HandleIdentifier {
    inode: 0,
    drive: 0,
};

#[derive(Debug, Clone, PartialEq, Copy, Serialize)]
pub enum FileState {
    NotProcessed,
    Analyzed,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInformation {
    pub id: HandleIdentifier,
    pub path: std::path::PathBuf,
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
    pub id: HandleIdentifier,
    pub path: std::path::PathBuf,
    pub modified: u64,
    pub content_hash: GeneralHash,
    pub number_of_children: u64,
    pub children: Vec<Rc<RefCell<FileContainer>>>,
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
    File(HandleIdentifier, Weak<RefCell<FileContainer>>), // if the symlink points to a file
    Path(PathBuf), // if follow symlinks is disabled, or path is outside the analysis scope
}

#[derive(Debug, Clone, Serialize)]
pub struct SymlinkInformation {
    pub id: HandleIdentifier,
    pub path: PathBuf,
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

#[derive(Debug, Clone, Serialize)]
pub enum FileContainer {
    InMemory(RefCell<File>),
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
            File::Other(_) => &NULL_HANDLE,
        }
    }

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
}

impl FileContainer {
    #[deprecated]
    pub fn id(&self) -> Option<HandleIdentifier> {
        match self {
            FileContainer::InMemory(file) => Some(file.borrow().id().clone()),
            FileContainer::OnDisk(id) => Some(id.clone()),
            FileContainer::DoesNotExist => None,
        }
    }

    pub fn has_finished_analyzing(&self) -> bool {
        match self {
            FileContainer::InMemory(file) => file.borrow().has_finished_analyzing(),
            FileContainer::OnDisk(_) => true, // files loaded out of memory are assumed to be fully processed
            FileContainer::DoesNotExist => true,
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
            FileContainer::OnDisk(_) => false, // files loaded out of memory are assumed to be fully processed
            FileContainer::DoesNotExist => false,
        }
    }
}

impl File {
    pub fn new(path: PathBuf, follow_symlinks: bool, inside_scope: PathInScopeFn, lookup_id: ResolveNodeFn) -> Self {
        let metadata_result = fs::symlink_metadata(&path);
        let metadata ;
        match metadata_result {
            Ok(meta) => metadata = meta,
            Err(err) => {
                warn!("Error while reading metadata {:?}: {}", path, err);
                return Self::Other(OtherInformation { path });
            }
        }
        let handle_result = same_file::Handle::from_path(&path);
        let handle;
        match handle_result {
            Ok(h) => handle = h,
            Err(err) => {
                warn!("Error while reading handle {:?}: {}", path, err);
                return Self::Other(OtherInformation { path });
            }
        }

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
                error!("Error while processing file {:?}: {}", path, err);
                error = true;
                modified = 0;
            }
        }

        if metadata.is_symlink() {
            let target_link_result = fs::read_link(&path);
            match target_link_result {
                Err(err) => {
                    error!("Error while reading symlink {:?}: {}", path, err);
                    return Self::Symlink(SymlinkInformation {
                        id: HandleIdentifier {
                            inode,
                            drive,
                        },
                        path,
                        modified,
                        content_hash: GeneralHash::new_sha256(),
                        target: SymlinkTarget::Path(PathBuf::new()),
                        state: SymlinkState::Error,
                    });
                }
                Ok(target_link) => {
                    let target;

                    if !follow_symlinks {
                        target = SymlinkTarget::Path(target_link);
                    } else if !inside_scope(&target_link) {
                        target = SymlinkTarget::Path(target_link);
                    } else {
                        let target_handle_result = same_file::Handle::from_path(&target_link);
                        let target_inode;
                        let target_drive;
                        match target_handle_result {
                            Ok(handle) => {
                                target_inode = handle.ino();
                                target_drive = handle.dev();
                            },
                            Err(err) => {
                                error!("Error while reading symlink target {:?}: {}", path, err);
                                return Self::Symlink(SymlinkInformation {
                                    id: HandleIdentifier {
                                        inode,
                                        drive,
                                    },
                                    path,
                                    modified,
                                    content_hash: GeneralHash::new_sha256(),
                                    target: SymlinkTarget::Path(PathBuf::new()),
                                    state: SymlinkState::NotProcessed,
                                });
                            }
                        }

                        let target_id = HandleIdentifier {
                            inode: target_inode,
                            drive: target_drive,
                        };

                        let looked_up_target_node_result = lookup_id(&target_id);
                        match looked_up_target_node_result {
                            Ok(strong) => {
                                target = SymlinkTarget::File(target_id, Rc::downgrade(&strong));
                            },
                            Err(err) => {
                                error!("Error while resolving target of symlink {:?} -> {:?}: {}", path, target_link, err);
                                debug!("This indicates an error in the lookup target function, the symlink will be marked as error");
                                target = SymlinkTarget::Path(target_link);
                                error = true;
                            }
                        }
                    }
                    Self::Symlink(SymlinkInformation {
                        id: HandleIdentifier {
                            inode,
                            drive,
                        },
                        path,
                        modified,
                        content_hash: GeneralHash::new_sha256(),
                        target,
                        state: if error { SymlinkState::Error } else { SymlinkState::NotProcessed },
                    })
                }
            }
        } else if metadata.is_dir() {
            Self::Directory(DirectoryInformation {
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
            })
        } else if metadata.is_file() {
            Self::File(FileInformation {
                id: HandleIdentifier {
                    inode,
                    drive,
                },
                path,
                modified,
                content_hash: GeneralHash::new_sha256(),
                content_size: 0,
                state: FileState::NotProcessed,
            })
        } else {
            Self::Other(OtherInformation { path })
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
                        error!("Error while hashing file {:?}: {}", self.path, err);
                        self.state = FileState::Error;
                    }
                }
            }
            Err(err) => {
                error!("Error while opening file {:?}: {}", self.path, err);
                self.state = FileState::Error;
            }
        }
    }
}

impl DirectoryInformation {
    pub fn analyze_expand(&mut self, follow_symlinks: bool, inside_scope: PathInScopeFn, lookup_id: ResolveNodeFn) {
        if self.state != DirectoryState::NotProcessed {
            return;
        }

        // list all children

        let read_dir_result = fs::read_dir(&self.path);

        match read_dir_result {
            Err(err) => {
                error!("Error while reading directory {:?}: {}", self.path, err);
                self.state = DirectoryState::Error;
                return;
            },
            Ok(read_dir) => {
                self.state = DirectoryState::Evaluating;
                for entry in read_dir {
                    let entry = match entry {
                        Err(err) => {
                            error!("Error while reading directory {:?}: {}", self.path, err);
                            self.state = DirectoryState::Error;
                            return;
                        },
                        Ok(entry) => entry,
                    };

                    let path = entry.path();
                    let file = File::new(path.clone(), follow_symlinks, inside_scope, lookup_id);
                    if let File::Other(_) = file {
                        warn!("Unsupported file type: {:?}", path);
                        self.children.push(Rc::new(RefCell::new(FileContainer::InMemory(RefCell::new(file)))));
                    } else {
                        self.children.push(Rc::new(RefCell::new(FileContainer::InMemory(RefCell::new(file)))));
                    }
                }
                self.state = DirectoryState::Analyzed;
                return;
            }
        }
    }
    pub fn analyze_collect(&mut self) {
        if self.state == DirectoryState::Evaluating {
            return;
        }

        // check if all children are analyzed

        for child in &self.children {
            let child = child.borrow(); // todo check
            if child.has_errored() {
                self.state = DirectoryState::Error;
                return;
            } else if !child.has_finished_analyzing() {
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
                error!("Error while hashing directory {:?}: {}", self.path, err);
                self.state = DirectoryState::Error;
            }
        }
    }
}

impl SymlinkInformation {
    pub fn analyze(&mut self, load_id: ResolveNodeFn) {
        if self.state != SymlinkState::NotProcessed {
            return;
        }

        match &self.target {
            SymlinkTarget::Path(path) => {
                let result = utils::hash_path(path, &mut self.content_hash);
                match result {
                    Ok(_) => {
                        self.state = SymlinkState::Analyzed;
                    }
                    Err(err) => {
                        error!("Error while hashing symlink {:?}: {}", self.path, err);
                        self.state = SymlinkState::Error;
                    }
                }
            },
            SymlinkTarget::File(target_id, weak) => {
                let strong = weak.upgrade();
                let target_ref;

                match strong {
                    None => {
                        debug!("Symlink target {:?} has been unloaded", self.path);
                        let target_load = load_id(target_id);
                        match target_load {
                            Ok(loaded) => {
                                target_ref = loaded;
                            }
                            Err(err) => {
                                error!("Error while loading symlink target {:?}: {}", self.path, err);
                                self.state = SymlinkState::Error;
                                return;
                            }
                        }
                    },
                    Some(loaded) => {
                        target_ref = loaded;
                    }
                }

                let target_id_load;
                let file = target_ref.borrow();
                match *file {
                    FileContainer::InMemory(ref file) => {
                        self.content_hash = file.borrow().get_content_hash().clone();
                        self.state = SymlinkState::Analyzed;
                        return;
                    },
                    FileContainer::OnDisk(ref target_id) => {
                        debug!("Symlink target {:?} is not loaded into memory", self.path);
                        target_id_load = target_id.clone();
                    },
                    FileContainer::DoesNotExist => {
                        error!("Symlink target {:?} does not exist", self.path);
                        self.state = SymlinkState::Error;
                        return;
                    }
                }
                drop(file);

                let result = load_id(&target_id_load);
                match result {
                    Ok(loaded) => {
                        let file = loaded.borrow();
                        match *file {
                            FileContainer::InMemory(ref file) => {
                                self.content_hash = file.borrow().get_content_hash().clone();
                                self.state = SymlinkState::Analyzed;
                            },
                            FileContainer::OnDisk(_) => {
                                error!("Target of symlink {:?} is still not loaded into memory", self.path);
                                self.state = SymlinkState::Error;
                            },
                            FileContainer::DoesNotExist => {
                                error!("Target of symlink {:?} does not exist", self.path);
                                self.state = SymlinkState::Error;
                            }
                        }
                    },
                    Err(err) => {
                        error!("Error while loading symlink target {:?}: {}", self.path, err);
                        self.state = SymlinkState::Error;
                    }
                }
            }
        }
    }
}