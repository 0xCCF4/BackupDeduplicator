use std::collections::HashMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use crate::data::{File, FilePath, JobTrait, ResultTrait, SaveFileEntry, SaveFileEntryType};
use super::analysis::{DirectoryInformation, AnalysisFile, FileInformation, OtherInformation, SymlinkInformation};

#[derive(Debug)]
pub struct MarkedIntermediaryFile {
    pub saved_file_entry: Arc<SaveFileEntry>,
    pub file: Arc<Mutex<Option<Arc<AnalysisFile>>>>,
}

pub struct WorkerArgument {
    pub file_by_path: Arc<HashMap<FilePath, MarkedIntermediaryFile>>,
}

#[derive(Debug)]
pub struct AnalysisJob {
    id: usize,
    pub file: Arc<SaveFileEntry>,
}

impl AnalysisJob {
    pub fn new(file: Arc<SaveFileEntry>) -> Self {
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
    file: Arc<Mutex<File>>,
}

impl ResultTrait for AnalysisResult {

}



fn parent_file(file: &MarkedIntermediaryFile) -> Option<&MarkedIntermediaryFile> {
    None
}

fn recursive_process_file(path: &FilePath, arg: &mut WorkerArgument) {
    /*
    let mut marked_file = arg.file_by_path.get(path);
    
    while let Some(file) = marked_file {
        let result = match file.saved_file_entry.file_type {
            SaveFileEntryType::File => {
                AnalysisFile::File(FileInformation {
                    path: file.saved_file_entry.path.clone(),
                    content_hash: file.saved_file_entry.hash.clone(),
                })
            },
            SaveFileEntryType::Symlink => {
                AnalysisFile::Symlink(SymlinkInformation {
                    path: file.saved_file_entry.path.clone(),
                    content_hash: file.saved_file_entry.hash.clone(),
                })
            },
            SaveFileEntryType::Other => {
                AnalysisFile::Other(OtherInformation {
                    path: file.saved_file_entry.path.clone(),
                })
            },
            SaveFileEntryType::Directory => {
                AnalysisFile::Directory(DirectoryInformation {
                    path: file.saved_file_entry.path.clone(),
                    content_hash: file.saved_file_entry.hash.clone(),
                    children: vec![],
                })
            }
        };
        let result = Arc::new(result);
        
        match file.file.lock() {
            Ok(mut guard) => {
                if guard.deref().is_none() {
                    *guard = Some(result);
                } else {
                    break;
                }
            },
            Err(err) => {
                panic!("Failed to lock file: {}", err);
            }
        }
        
        let parent = parent_file(file);
        
        if let Some(parent) = parent {
            match parent.file.lock() {
                Ok(mut guard) => {
                    if let Some(parent) = guard.deref() {
                        match parent {
                            File::Directory(parent) => {
                                parent.children.push()
                            }
                        }
                    }
                },
                Err(err) => {
                    panic!("Failed to lock file: {}", err);
                }
            }
        }
        
        marked_file = parent;
    }
    */
    
    todo!("Implement recursive_process_file");
}

pub fn worker_run(id: usize, job: AnalysisJob, result_publish: &Sender<AnalysisResult>, job_publish: &Sender<AnalysisJob>, arg: &mut WorkerArgument) {
    todo!("Implement worker_run")
}