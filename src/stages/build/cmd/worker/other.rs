use crate::stages::build::cmd::job::{BuildJob, JobResult};
use crate::stages::build::cmd::worker::{
    worker_fetch_savedata, worker_publish_result_or_trigger_parent, WorkerArgument,
};
use crate::stages::build::intermediary_build_data::{BuildFile, BuildOtherInformation};
use crate::stages::build::output::HashTreeFileEntryType;
use log::trace;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Arguments of the [worker_run_other] function.
///
/// # Fields
/// * `path` - The path to the file.
/// * `modified` - The last modified time of the file.
/// * `size` - The size of the file (given by fs::metadata).
/// * `id` - The id of the worker.
/// * `job` - The job to process.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
pub struct WorkerRunOtherArguments<'a, 'b, 'c> {
    /// The path to the file.
    pub path: PathBuf,
    /// The last modified time of the file.
    pub modified: u64,
    /// The size of the file (given by fs::metadata).
    pub size: u64,
    /// The id of the worker.
    pub id: usize,
    /// The job to process.
    pub job: BuildJob,
    /// The channel to publish the result to.
    pub result_publish: &'a Sender<JobResult>,
    /// The channel to publish new jobs to.
    pub job_publish: &'b Sender<BuildJob>,
    /// The argument for the worker thread.
    pub arg: &'c mut WorkerArgument,
}

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
pub fn worker_run_other(arguments: WorkerRunOtherArguments) {
    let WorkerRunOtherArguments {
        path,
        modified,
        size,
        id,
        job,
        result_publish,
        job_publish,
        arg,
    } = arguments;

    trace!("[{}] analyzing other {} > {:?}", id, &job.target_path, path);

    if let Some(found) = worker_fetch_savedata(arg, &job.target_path) {
        if found.file_type == HashTreeFileEntryType::Other
            && found.modified == modified
            && found.size == size
        {
            trace!("Other {:?} is already in save file", path);
            worker_publish_result_or_trigger_parent(
                id,
                true,
                BuildFile::Other(BuildOtherInformation {
                    path: job.target_path.clone(),
                    content_size: size,
                    modified,
                }),
                job,
                result_publish,
                job_publish,
                arg,
            );
            return;
        }
    }

    let file = BuildFile::Other(BuildOtherInformation {
        path: job.target_path.clone(),
        content_size: size,
        modified,
    });

    worker_publish_result_or_trigger_parent(id, false, file, job, result_publish, job_publish, arg);
}
