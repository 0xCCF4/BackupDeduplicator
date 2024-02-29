use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

impl File {
    pub fn new(path: PathBuf, /* follow_symlinks: bool, inside_scope: PathInScopeFn, lookup_id: ResolveNodeFn */) -> Self {
        let metadata_result = fs::symlink_metadata(&path);
        let metadata ;
        match metadata_result {
            Ok(meta) => metadata = meta,
            Err(err) => {
                warn!("Error while reading metadata {:?}: {}", path, err);
                return Self::Other(OtherInformation { path });
            }
        }
        let handle_result = fileid::from_path(&path);
        let handle;
        match handle_result {
            Ok(h) => handle = h,
            Err(err) => {
                warn!("Error while reading handle {:?}: {}", path, err);
                return Self::Other(OtherInformation { path });
            }
        }

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
                        id: handle,
                        path,
                        modified,
                        content_hash: GeneralHash::new_sha256(),
                        target: SymlinkTarget::Path(PathBuf::new()),
                        state: SymlinkState::Error,
                    });
                }
                Ok(target_link) => {
                    let target;

                    let follow_symlinks = false; // todo add symlinks
                    let inside_scope = |_path: &'_ Path| -> bool { false };

                    if !follow_symlinks {
                        target = SymlinkTarget::Path(target_link);
                    } else if !inside_scope(&target_link) {
                        target = SymlinkTarget::Path(target_link);
                    } else {
                        let lookup_id = |_id: &'_ HandleIdentifier| -> Result<anyhow::Error> { Err(anyhow!("lookup_id")) };

                        let target_handle_result = fileid::from_path(&target_link);
                        let target_id;
                        match target_handle_result {
                            Ok(handle) => {
                                target_id = handle;
                            },
                            Err(err) => {
                                error!("Error while reading symlink target {:?}: {}", path, err);
                                return Self::Symlink(SymlinkInformation {
                                    id: handle,
                                    path,
                                    modified,
                                    content_hash: GeneralHash::new_sha256(),
                                    target: SymlinkTarget::Path(PathBuf::new()),
                                    state: SymlinkState::NotProcessed,
                                });
                            }
                        }

                        let looked_up_target_node_result = lookup_id(&target_id);
                        match looked_up_target_node_result {
                            Ok(_strong) => {
                                todo!("todo symlinks");
                                //target = SymlinkTarget::File(target_id, Rc::downgrade(&strong));
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
                        id: handle,
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
                id: handle,
                path,
                modified,
                number_of_children: 0,
                content_hash: GeneralHash::new_sha256(),
                children: Vec::new(),
                state: DirectoryState::NotProcessed,
            })
        } else if metadata.is_file() {
            Self::File(FileInformation {
                id: handle,
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
    pub fn analyze_expand(&mut self/*, follow_symlinks: bool, inside_scope: PathInScopeFn, lookup_id: ResolveNodeFn*/) {
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
                    let file = File::new(path.clone()/*, follow_symlinks, inside_scope, lookup_id*/);
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

    pub fn get_next_unfinshed_child(&self) -> Option<Rc<RefCell<FileContainer>>> {
        for child in &self.children {
            let child_borrow = child.borrow();
            if child_borrow.has_errored() {
                return None;
            } else if !child_borrow.has_finished_analyzing() {
                return Some(Rc::clone(child));
            }
        }
        None
    }
}

impl SymlinkInformation {
    pub fn analyze(&mut self/*, load_id: ResolveNodeFn*/) {
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
            /* SymlinkTarget::File(target_id, weak) => {
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
                        let file_borrow = file.borrow();
                        if !file_borrow.has_finished_analyzing() {
                            debug!("Symlink target {:?} is not analyzed yet", self.path);
                            return;
                        }
                        self.content_hash = file.borrow().get_content_hash().clone();
                        self.state = SymlinkState::Analyzed;
                        return;
                    },
                    /*FileContainer::OnDisk(ref target_id) => {
                        debug!("Symlink target {:?} is not loaded into memory", self.path);
                        target_id_load = target_id.clone();
                    },*/
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
                            /*FileContainer::OnDisk(_) => {
                                error!("Target of symlink {:?} is still not loaded into memory", self.path);
                                self.state = SymlinkState::Error;
                            },*/
                        }
                    },
                    Err(err) => {
                        error!("Error while loading symlink target {:?}: {}", self.path, err);
                        self.state = SymlinkState::Error;
                    }
                }
            } */
        }
    }
}
