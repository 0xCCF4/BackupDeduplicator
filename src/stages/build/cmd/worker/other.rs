use std::path::PathBuf;
use std::sync::mpsc::Sender;
use log::trace;
use crate::stages::build::intermediary_build_data::{BuildFile, BuildOtherInformation};
use crate::stages::build::cmd::job::{BuildJob, JobResult};
use crate::stages::build::cmd::worker::{worker_fetch_savedata, worker_publish_result_or_trigger_parent, WorkerArgument};
use crate::stages::build::output::HashTreeFileEntryType;

/// Analyze a file that is not a symlink/folder/file.
/// 
/// # Arguments
/// * `path` - The path to the file.
/// * `modified` - The last modified time of the file.
/// * `size` - The size of the file (given by fs::metadata).
/// * `id` - The id of the worker.
/// * `job` - The job to process.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
pub fn worker_run_other(path: PathBuf, modified: u64, size: u64, id: usize, job: BuildJob, result_publish: &Sender<JobResult>, job_publish: &Sender<BuildJob>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing other {} > {:?}", id, &job.target_path, path);

    match worker_fetch_savedata(arg, &job.target_path) {
        Some(found) => {
            if found.file_type == HashTreeFileEntryType::Other && found.modified == modified && found.size == size {
                trace!("Other {:?} is already in save file", path);
                worker_publish_result_or_trigger_parent(id, true, BuildFile::Other(BuildOtherInformation {
                    path: job.target_path.clone(),
                    content_size: size,
                    modified,
                }), job, result_publish, job_publish, arg);
                return;
            }
        }
        None => {}
    }
    
    let file = BuildFile::Other(BuildOtherInformation {
        path: job.target_path.clone(),
        content_size: size,
        modified,
    });

    worker_publish_result_or_trigger_parent(id, false, file, job, result_publish, job_publish, arg);
}