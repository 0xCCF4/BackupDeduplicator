use crate::hash::GeneralHash;
use crate::stages::build::cmd::job::JobResult;
use crate::stages::build::cmd::worker::BuildJob;
use crate::stages::build::cmd::worker::{
    worker_create_error, worker_fetch_savedata, worker_publish_result_or_trigger_parent,
    WorkerArgument,
};
use crate::stages::build::intermediary_build_data::{BuildFile, BuildSymlinkInformation};
use crate::stages::build::output::HashTreeFileEntryType;
use log::{error, trace};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Analyze a symlink.
///
/// # Arguments
/// * `path` - The path to the symlink.
/// * `modified` - The last modified time of the symlink.
/// * `size` - The size of the symlink (given by fs::metdata).
/// * `id` - The id of the worker.
/// * `job` - The job to process.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
pub fn worker_run_symlink(
    path: PathBuf,
    modified: u64,
    size: u64,
    id: usize,
    job: BuildJob,
    result_publish: &Sender<JobResult>,
    job_publish: &Sender<BuildJob>,
    arg: &mut WorkerArgument,
) {
    trace!(
        "[{}] analyzing symlink {} > {:?}",
        id,
        &job.target_path,
        path
    );

    match worker_fetch_savedata(arg, &job.target_path) {
        Some(found) => {
            if found.file_type == HashTreeFileEntryType::Symlink
                && found.modified == modified
                && found.size == size
            {
                trace!("Symlink {:?} is already in save file", path);
                let target_link = fs::read_link(&path);
                let target_link = match target_link {
                    Ok(target_link) => target_link,
                    Err(err) => {
                        error!("Error while reading symlink {:?}: {}", path, err);
                        worker_publish_result_or_trigger_parent(
                            id,
                            false,
                            worker_create_error(job.target_path.clone(), modified, size),
                            job,
                            result_publish,
                            job_publish,
                            arg,
                        );
                        return;
                    }
                };
                worker_publish_result_or_trigger_parent(
                    id,
                    true,
                    BuildFile::Symlink(BuildSymlinkInformation {
                        path: job.target_path.clone(),
                        modified,
                        content_hash: found.hash.clone(),
                        target: target_link,
                        content_size: size,
                    }),
                    job,
                    result_publish,
                    job_publish,
                    arg,
                );
                return;
            }
        }
        None => {}
    }

    let target_link = fs::read_link(&path);
    let target_link = match target_link {
        Ok(target_link) => target_link,
        Err(err) => {
            error!("Error while reading symlink {:?}: {}", path, err);
            worker_publish_result_or_trigger_parent(
                id,
                false,
                worker_create_error(job.target_path.clone(), modified, size),
                job,
                result_publish,
                job_publish,
                arg,
            );
            return;
        }
    };

    let mut hash = GeneralHash::from_type(arg.hash_type);

    match hash.hash_path(&target_link) {
        Ok(_) => {}
        Err(err) => {
            error!(
                "Error while hashing symlink target {:?}: {}",
                target_link, err
            );
            worker_publish_result_or_trigger_parent(
                id,
                false,
                worker_create_error(job.target_path.clone(), modified, size),
                job,
                result_publish,
                job_publish,
                arg,
            );
            return;
        }
    }

    let file = BuildFile::Symlink(BuildSymlinkInformation {
        path: job.target_path.clone(),
        modified,
        content_hash: hash,
        target: target_link,
        content_size: size,
    });

    worker_publish_result_or_trigger_parent(id, false, file, job, result_publish, job_publish, arg);
}
