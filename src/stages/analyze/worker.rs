use crate::path::FilePath;
use crate::pool::{JobTrait, ResultTrait};
use crate::stages::analyze::intermediary_analysis_data::{
    AnalysisDirectoryInformation, AnalysisFile, AnalysisFileInformation, AnalysisOtherInformation,
    AnalysisSymlinkInformation,
};
use crate::stages::build::output::{HashTreeFileEntry, HashTreeFileEntryType};
use log::error;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// The intermediary file for the analysis worker.
///
/// # Fields
/// * `saved_file_entry` - A saved file entry from the hash tree file.
/// * `file` - Analysis result of the file. Processed by a worker.
#[derive(Debug)]
pub struct AnalysisIntermediaryFile {
    pub saved_file_entry: Arc<HashTreeFileEntry>,
    pub file: Arc<Mutex<Option<Arc<AnalysisFile>>>>,
}

/// The argument for the analysis worker main thread.
/// Files from the hash tree file are stored in a hash map.
///
/// # Fields
/// * `file_by_path` - A hash map of [FilePath] -> [AnalysisIntermediaryFile].
pub struct AnalysisWorkerArgument {
    pub file_by_path: Arc<HashMap<FilePath, AnalysisIntermediaryFile>>,
}

/// The job for the analysis worker.
///
/// # Fields
/// * `id` - The id of the job.
/// * `file` - The file to analyze.
#[derive(Debug)]
pub struct AnalysisJob {
    id: usize,
    pub file: Arc<HashTreeFileEntry>,
}

impl AnalysisJob {
    /// Create a new analysis job.
    ///
    /// # Arguments
    /// * `file` - The file to analyze.
    ///
    /// # Returns
    /// The analysis job.
    pub fn new(file: Arc<HashTreeFileEntry>) -> Self {
        Self {
            id: new_job_counter_id(),
            file,
        }
    }
}

impl JobTrait for AnalysisJob {
    /// Get the job id.
    ///
    /// # Returns
    /// The job id.
    fn job_id(&self) -> usize {
        self.id
    }
}

static JOB_COUNTER: Mutex<usize> = Mutex::new(0);

fn new_job_counter_id() -> usize {
    let mut counter = JOB_COUNTER.lock().expect("Failed to lock job counter");
    *counter += 1;
    *counter
}

/// The result for the analysis worker.
#[derive(Debug)]
pub struct AnalysisResult {}

impl ResultTrait for AnalysisResult {}

/// Get the parent file of a file. Searches the arg.cache for the parent file.
///
/// # Arguments
/// * `file` - The file to get the parent of.
/// * `arg` - The argument for the worker thread.
///
/// # Returns
/// The parent file and the parent path.
/// If the parent file is not present, return None.
#[allow(clippy::type_complexity)] // non-public function
fn parent_file<'a>(
    file: &AnalysisIntermediaryFile,
    arg: &'a AnalysisWorkerArgument,
) -> Option<(&'a Arc<Mutex<Option<Arc<AnalysisFile>>>>, FilePath)> {
    match file.saved_file_entry.path.parent() {
        None => None,
        Some(parent_path) => {
            let cache = arg.file_by_path.get(&parent_path).map(|file| &file.file);
            cache.map(|cache| (cache, parent_path))
        }
    }
}

/// Recursively process a file. Iterates over the file and its parent files until
/// the parent file is present or the root is reached.
///
/// # Arguments
/// * `id` - The id of the worker.
/// * `path` - The path of the file to process.
/// * `arg` - The argument for the worker thread.
fn recursive_process_file(id: usize, path: &FilePath, arg: &AnalysisWorkerArgument) {
    let marked_file = arg.file_by_path.get(path);

    let mut attach_parent = None;

    if let Some(file) = marked_file {
        let result = match file.saved_file_entry.file_type {
            HashTreeFileEntryType::File => AnalysisFile::File(AnalysisFileInformation {
                path: file.saved_file_entry.path.clone(),
                content_hash: file.saved_file_entry.hash.clone(),
                parent: Mutex::new(None),
            }),
            HashTreeFileEntryType::Symlink => AnalysisFile::Symlink(AnalysisSymlinkInformation {
                path: file.saved_file_entry.path.clone(),
                content_hash: file.saved_file_entry.hash.clone(),
                parent: Mutex::new(None),
            }),
            HashTreeFileEntryType::Other => AnalysisFile::Other(AnalysisOtherInformation {
                path: file.saved_file_entry.path.clone(),
                parent: Mutex::new(None),
            }),
            HashTreeFileEntryType::Directory => {
                AnalysisFile::Directory(AnalysisDirectoryInformation {
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
            }
            Err(err) => {
                panic!("[{}] Failed to lock file: {}", id, err);
            }
        }

        if let Some((parent, parent_path)) = parent_file(file, arg) {
            attach_parent = Some((result, parent, parent_path));
        }
    }

    if let Some((result, parent, parent_path)) = attach_parent {
        match add_to_parent_as_child(id, parent, &result) {
            AddToParentResult::Ok => {}
            AddToParentResult::ParentDoesNotExist => {
                // parent does not exist
                // create it
                recursive_process_file(id, &parent_path, arg);
                // try to read to parent again
                match add_to_parent_as_child(id, parent, &result) {
                    AddToParentResult::Ok => {}
                    AddToParentResult::ParentDoesNotExist => {
                        error!("[{}] Parent still does not exist", id);
                    }
                    AddToParentResult::Error => {}
                }
            }
            AddToParentResult::Error => {}
        }
    }
}

/// The result of adding a file to a parent as child, see [add_to_parent_as_child]
///
/// # Variants
/// * `Ok` - The operation was successful.
/// * `ParentDoesNotExist` - The parent does not exist.
/// * `Error` - An error occurred during the operation
enum AddToParentResult {
    Ok,
    ParentDoesNotExist,
    Error,
}

/// Add a file to a parent as a child.
///
/// # Arguments
/// * `id` - The id of the worker.
/// * `parent` - The parent file.
/// * `child` - The child file.
///
/// # Returns
/// The result of the operation.
fn add_to_parent_as_child(
    id: usize,
    parent: &Arc<Mutex<Option<Arc<AnalysisFile>>>>,
    child: &Arc<AnalysisFile>,
) -> AddToParentResult {
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
                        }
                        Err(err) => {
                            error!("[{}] Failed to lock parent: {}", id, err);
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
                                }
                                Err(err) => {
                                    error!("[{}] Failed to lock children: {}", id, err);
                                    AddToParentResult::Error
                                }
                            }
                        }
                        _ => {
                            error!("[{}] Parent is not a directory", id);
                            AddToParentResult::Error
                        }
                    }
                }
                None => {
                    // parent not yet present
                    AddToParentResult::ParentDoesNotExist
                }
            }
        }
        Err(err) => {
            error!("[{}] Failed to lock file: {}", id, err);
            AddToParentResult::Error
        }
    }
}

/// The main function for the analysis worker.
///
/// # Arguments
pub fn worker_run(
    id: usize,
    job: AnalysisJob,
    _result_publish: &Sender<AnalysisResult>,
    _job_publish: &Sender<AnalysisJob>,
    arg: &mut AnalysisWorkerArgument,
) {
    recursive_process_file(id, &job.file.path, arg);
}
