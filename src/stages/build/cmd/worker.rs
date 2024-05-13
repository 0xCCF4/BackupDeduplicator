use crate::stages::build::intermediary_build_data::{BuildFile, BuildOtherInformation, BuildStubInformation};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::SystemTime;
use anyhow::{anyhow};
use log::{error, info, trace, warn};
use crate::hash::GeneralHashType;
use crate::path::FilePath;
use crate::stages::build::cmd::job::{BuildJob, JobResult, JobResultContent};
use crate::stages::build::cmd::worker::directory::worker_run_directory;
use crate::stages::build::cmd::worker::file::worker_run_file;
use crate::stages::build::cmd::worker::other::worker_run_other;
use crate::stages::build::cmd::worker::symlink::worker_run_symlink;
use crate::stages::build::output::HashTreeFileEntry;

mod directory;
mod file;
mod other;
mod symlink;
mod archive;

/// The argument for the worker main thread.
/// 
/// # Fields
/// * `follow_symlinks` - Whether to follow symlinks when traversing the file system.
/// * `archives` - Whether to traverse into archives.
/// * `hash_type` - The hash algorithm to use for hashing files.
/// * `save_file_by_path` - A hash map of [FilePath] -> [HashTreeFileEntry].
pub struct WorkerArgument {
    pub follow_symlinks: bool,
    pub archives: bool,
    pub hash_type: GeneralHashType,
    pub save_file_by_path: Arc<HashMap<FilePath, HashTreeFileEntry>>,
}

/// Main function for the worker thread.
/// 
/// # Arguments
/// * `id` - The id of the worker.
/// * `job` - The job to process.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
pub fn worker_run(id: usize, job: BuildJob, result_publish: &Sender<JobResult>, job_publish: &Sender<BuildJob>, arg: &mut WorkerArgument) {
    let path = job.target_path.resolve_file();
    let path = match path {
        Ok(file) => file,
        Err(e) => {
            error!("[{}] failed to resolve file: {}", id, e);
            info!("[{}] Skipping file...", id);
            worker_publish_result_or_trigger_parent(id, false, worker_create_error(job.target_path.clone(), 0, 0), job, result_publish, job_publish, arg);
            return;
        }
    };

    let metadata = match arg.follow_symlinks {
        true => fs::metadata(&path),
        false => fs::symlink_metadata(&path),
    };

    let metadata = match metadata {
        Ok(metadata) => metadata,
        Err(e) => {
            warn!("[{}] failed to read metadata: {}", id, e);
            info!("[{}] Skipping file...", id);
            worker_publish_result_or_trigger_parent(id, false, worker_create_error(job.target_path.clone(), 0, 0), job, result_publish, job_publish, arg);
            return;
        }
    };

    let modified_result = metadata.modified()
        .map(|time| time.duration_since(SystemTime::UNIX_EPOCH)
            .or(Err(anyhow!("Unable to convert modified date to UNIX_EPOCH")))
            .map(|duration| duration.as_secs())
        ).unwrap_or_else(|err| {
        error!("Error while reading modified date {:?}: {:?}", path, err);
        Ok(0)
    });

    let modified;
    
    let size = metadata.len();
    
    match modified_result {
        Ok(time) => modified = time,
        Err(err) => {
            error!("Error while processing file {:?}: {}", path, err);
            modified = 0;
        }
    }

    if metadata.is_symlink() {
        worker_run_symlink(path, modified, size, id, job, result_publish, job_publish, arg);
    } else if metadata.is_dir() {
        worker_run_directory(path, modified, size, id, job, result_publish, job_publish, arg);
    } else if metadata.is_file() {
        worker_run_file(path, modified, size, id, job, result_publish, job_publish, arg);
    } else {
        worker_run_other(path, modified, size, id, job, result_publish, job_publish, arg);
    }
}

/// Publish a result to the result channel.
/// Processes the error if the result could not be published.
/// 
/// # Error
/// Never, issues a warning instead
fn worker_publish_result(id: usize, result_publish: &Sender<JobResult>, result: JobResult) {
    match result_publish.send(result) {
        Ok(_) => {},
        Err(e) => {
            warn!("[{}] failed to publish result: {}", id, e);
        }
    }
}

/// Create a [File::Other] with the given information.
/// Used when an error occurs.
/// 
/// # Arguments
/// * `path` - The path of the file.
/// * `modified` - The modified date of the file.
/// * `size` - The size of the file.
/// 
/// # Returns
/// The created [File::Other].
fn worker_create_error(path: FilePath, modified: u64, size: u64) -> BuildFile {
    BuildFile::Other(BuildOtherInformation {
        path,
        modified,
        content_size: size,
    })
}

/// Publish a new job.
/// 
/// # Arguments
/// * `id` - The id of the worker.
/// * `job_publish` - The channel to publish the job to.
/// * `job` - The job to publish.
/// 
/// # Error
/// Never, issues a warning instead
fn worker_publish_new_job(id: usize, job_publish: &Sender<BuildJob>, job: BuildJob) {
    match job_publish.send(job) {
        Ok(_) => {},
        Err(e) => {
            warn!("[{}] failed to publish job: {}", id, e);
        }
    }
}

/// Publish a result and trigger the parent job.
/// 
/// # Arguments
/// * `id` - The id of the worker.
/// * `cached` - Whether the file is already cached.
/// * `result` - The result to publish.
/// * `job` - The job that was processed.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
fn worker_publish_result_or_trigger_parent(id: usize, cached: bool, result: BuildFile, job: BuildJob, result_publish: &Sender<JobResult>, job_publish: &Sender<BuildJob>, _arg: &mut WorkerArgument) {
    let parent_job;

    let hash;

    match job.parent {
        Some(parent) => {
            parent_job = parent;
            hash = result.get_content_hash().to_owned();
            worker_publish_result(id, result_publish, JobResult::Intermediate(JobResultContent {already_cached: cached, content: result}));
        },
        None => {
            worker_publish_result(id, result_publish, JobResult::Final(JobResultContent {already_cached: cached, content: result}));
            return;
        },
    }

    match parent_job.finished_children.lock() {
        Ok(mut finished) => {
            finished.push(BuildFile::Stub(BuildStubInformation {
                path: job.target_path,
                content_hash: hash,
            }));
        },
        Err(err) => {
            error!("[{}] failed to lock finished children: {}", id, err);
        }
    }

    match Arc::into_inner(parent_job) {
        Some(parent_job) => {
            trace!("[{}] finished last child of parent {:?}", id, parent_job.target_path);
            let parent_job= parent_job.new_job_id();
            worker_publish_new_job(id, job_publish, parent_job);
        },
        None => {
            trace!("[{}] there are still open job, skip parent", id);
        }
    }
}

/// Fetch the saved data for a file.
/// 
/// # Arguments
/// * `args` - The argument for the worker thread.
/// * `path` - The path of the file to fetch the saved data for.
/// 
/// # Returns
/// The saved data for the file if it exists.
fn worker_fetch_savedata<'a, 'b>(args: &'a WorkerArgument, path: &'b FilePath) -> Option<&'a HashTreeFileEntry> {
    args.save_file_by_path.get(path)
}
