use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use log::error;
use crate::path::FilePath;
use crate::pool::{JobTrait, ResultTrait};
use crate::stages::analyze::output::{AnalysisFile, DirectoryInformation, FileInformation, OtherInformation, SymlinkInformation};
use crate::stages::build::output::{HashTreeFileEntry, HashTreeFileEntryType};

#[derive(Debug)]
pub struct MarkedIntermediaryFile {
    pub saved_file_entry: Arc<HashTreeFileEntry>,
    pub file: Arc<Mutex<Option<Arc<AnalysisFile>>>>,
}

pub struct WorkerArgument {
    pub file_by_path: Arc<HashMap<FilePath, MarkedIntermediaryFile>>,
}

#[derive(Debug)]
pub struct AnalysisJob {
    id: usize,
    pub file: Arc<HashTreeFileEntry>,
}

impl AnalysisJob {
    pub fn new(file: Arc<HashTreeFileEntry>) -> Self {
        Self {
            id: new_job_counter_id(),
            file,
        }
    }
}

static JOB_COUNTER: Mutex<usize> = Mutex::new(0);

fn new_job_counter_id() -> usize {
    let mut counter = JOB_COUNTER.lock().expect("Failed to lock job counter");
    *counter += 1;
    (*counter).clone()
}

impl JobTrait for AnalysisJob {
    fn job_id(&self) -> usize {
        self.id
    }
}

#[derive(Debug)]
pub struct AnalysisResult {
    
}

impl ResultTrait for AnalysisResult {

}



fn parent_file<'a, 'b>(file: &'b MarkedIntermediaryFile, arg: &'a WorkerArgument) -> Option<(&'a Arc<Mutex<Option<Arc<AnalysisFile>>>>, FilePath)> {
    match file.saved_file_entry.path.parent() {
        None => None,
        Some(parent_path) => {
            let cache = arg.file_by_path.get(&parent_path).map(|file| &file.file);
            match cache {
                None => {
                    None
                },
                Some(cache) => {
                    Some((cache, parent_path))
                }
            }
        }
    }
}

fn recursive_process_file(path: &FilePath, arg: &WorkerArgument) {
    let marked_file = arg.file_by_path.get(path);
    
    let mut attach_parent = None;
    
    if let Some(file) = marked_file {
        let result = match file.saved_file_entry.file_type {
            HashTreeFileEntryType::File => {
                AnalysisFile::File(FileInformation {
                    path: file.saved_file_entry.path.clone(),
                    content_hash: file.saved_file_entry.hash.clone(),
                    parent: Mutex::new(None),
                })
            },
            HashTreeFileEntryType::Symlink => {
                AnalysisFile::Symlink(SymlinkInformation {
                    path: file.saved_file_entry.path.clone(),
                    content_hash: file.saved_file_entry.hash.clone(),
                    parent: Mutex::new(None),
                })
            },
            HashTreeFileEntryType::Other => {
                AnalysisFile::Other(OtherInformation {
                    path: file.saved_file_entry.path.clone(),
                    parent: Mutex::new(None),
                })
            },
            HashTreeFileEntryType::Directory => {
                AnalysisFile::Directory(DirectoryInformation {
                    path: file.saved_file_entry.path.clone(),
                    content_hash: file.saved_file_entry.hash.clone(),
                    children: Mutex::new(Vec::new()),
                    parent: Mutex::new(None),
                })
            }
        };
        let result = Arc::new(result);

        match file.file.lock() {
            Ok(mut guard) => {
                if guard.deref().is_none() {
                    *guard = Some(Arc::clone(&result));
                } else {
                    return; // is already some
                }
            },
            Err(err) => {
                panic!("Failed to lock file: {}", err);
            }
        }

        if let Some((parent, parent_path)) = parent_file(file, arg) {
            attach_parent = Some((result, parent, parent_path));
        }
    }
    
    if let Some((result, parent, parent_path)) = attach_parent {
        match add_to_parent_as_child(parent, &result) {
            AddToParentResult::Ok => { return; },
            AddToParentResult::ParentDoesNotExist => {
                // parent does not exist
                // create it
                recursive_process_file(&parent_path, arg);
                // try to read to parent again
                match add_to_parent_as_child(parent, &result) {
                    AddToParentResult::Ok => { return; },
                    AddToParentResult::ParentDoesNotExist => {
                        error!("Parent still does not exist");
                        return;
                    },
                    AddToParentResult::Error => {
                        return;
                    }
                }
            },
            AddToParentResult::Error => {
                return;
            }
        }
    }
}

enum AddToParentResult {
    Ok,
    ParentDoesNotExist,
    Error,
}

fn add_to_parent_as_child(parent: &Arc<Mutex<Option<Arc<AnalysisFile>>>>, child: &Arc<AnalysisFile>) -> AddToParentResult {
    match parent.lock() {
        Ok(guard) => {
            // exclusive access to parent file
            match guard.deref() {
                Some(parent) => {
                    // parent already present
                    
                    match child.parent().lock() {
                        Ok(mut guard) => {
                            // set parent
                            *guard = Some(Arc::downgrade(parent));
                        },
                        Err(err) => {
                            error!("Failed to lock parent: {}", err);
                            return AddToParentResult::Error;
                        }
                    }
                    
                    match parent.deref() {
                        AnalysisFile::Directory(dir) => {
                            match dir.children.lock() {
                                Ok(mut guard) => {
                                    // add as child
                                    guard.push(Arc::clone(child));
                                    AddToParentResult::Ok
                                },
                                Err(err) => {
                                    error!("Failed to lock children: {}", err);
                                    AddToParentResult::Error
                                }
                            }
                        },
                        _ => {
                            error!("Parent is not a directory");
                            AddToParentResult::Error
                        }
                    }
                },
                None => {
                    // parent not yet present
                    AddToParentResult::ParentDoesNotExist
                }
            }
        },
        Err(err) => {
            error!("Failed to lock file: {}", err);
            AddToParentResult::Error
        }
    }
}

pub fn worker_run(_id: usize, job: AnalysisJob, _result_publish: &Sender<AnalysisResult>, _job_publish: &Sender<AnalysisJob>, arg: &mut WorkerArgument) {
    recursive_process_file(&job.file.path, arg);
}
