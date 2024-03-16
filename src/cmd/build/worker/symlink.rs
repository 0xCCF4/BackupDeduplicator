use crate::data::{File, SymlinkInformation};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use log::{error, trace};
use crate::build::JobResult;
use crate::build::worker::{worker_create_error, worker_publish_result_or_trigger_parent, WorkerArgument};
use crate::data::{GeneralHash, Job};
use crate::utils;

pub fn worker_run_symlink(path: PathBuf, modified: u64, id: usize, job: Job, result_publish: &Sender<JobResult>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing symlink {:?}#{:?}", id, &job.target_path, path);
    let target_link = fs::read_link(&path);
    let target_link = match target_link {
        Ok(target_link) => target_link,
        Err(err) => {
            error!("Error while reading symlink {:?}: {}", path, err);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
            return;
        }
    };

    let mut hash = GeneralHash::from_type(arg.hash);

    match utils::hash_path(&target_link, &mut hash) {
        Ok(_) => {},
        Err(err) => {
            error!("Error while hashing symlink target {:?}: {}", target_link, err);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
            return;
        }
    }

    let file = File::Symlink(SymlinkInformation {
        path: job.target_path.clone(),
        modified,
        content_hash: hash,
        target: target_link,
    });

    worker_publish_result_or_trigger_parent(id, file, job, result_publish, job_publish, arg);
}