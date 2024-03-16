use std::fs;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::SystemTime;
use anyhow::anyhow;
use log::{error, info, trace, warn};
use crate::build::JobResult;
use crate::build::worker::directory::worker_run_directory;
use crate::build::worker::file::worker_run_file;
use crate::build::worker::other::worker_run_other;
use crate::build::worker::symlink::worker_run_symlink;
use crate::data::{File, FilePath, GeneralHashType, Job, OtherInformation, StubInformation};

mod directory;
mod file;
mod other;
mod symlink;

pub struct WorkerArgument {
    pub follow_symlinks: bool,

    pub hash: GeneralHashType,
}

pub fn worker_run(id: usize, job: Job, result_publish: &Sender<JobResult>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    let path = job.target_path.resolve_file();
    let path = match path {
        Ok(file) => file,
        Err(e) => {
            error!("[{}] failed to resolve file: {}", id, e);
            info!("[{}] Skipping file...", id);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
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
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
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

    match modified_result {
        Ok(time) => modified = time,
        Err(err) => {
            error!("Error while processing file {:?}: {}", path, err);
            modified = 0;
        }
    }

    if metadata.is_symlink() {
        worker_run_symlink(path, modified, id, job, result_publish, job_publish, arg);
    } else if metadata.is_dir() {
        worker_run_directory(path, modified, id, job, result_publish, job_publish, arg);
    } else if metadata.is_file() {
        worker_run_file(path, modified, id, job, result_publish, job_publish, arg);
    } else {
        worker_run_other(path, modified, id, job, result_publish, job_publish, arg);
    }
}

fn worker_publish_result(id: usize, result_publish: &Sender<JobResult>, result: JobResult) {
    match result_publish.send(result) {
        Ok(_) => {},
        Err(e) => {
            warn!("[{}] failed to publish result: {}", id, e);
        }
    }
}

fn worker_create_error(path: FilePath) -> File {
    File::Other(OtherInformation {
        path,
    })
}

fn worker_publish_new_job(id: usize, job_publish: &Sender<Job>, job: Job) {
    match job_publish.send(job) {
        Ok(_) => {},
        Err(e) => {
            warn!("[{}] failed to publish job: {}", id, e);
        }
    }
}

fn worker_publish_result_or_trigger_parent(id: usize, result: File, job: Job, result_publish: &Sender<JobResult>, job_publish: &Sender<Job>, _arg: &mut WorkerArgument) {
    let parent_job;

    let hash;

    match job.parent {
        Some(parent) => {
            parent_job = parent;
            hash = result.get_content_hash().to_owned();
            worker_publish_result(id, result_publish, JobResult::Intermediate(result));
        },
        None => {
            worker_publish_result(id, result_publish, JobResult::Final(result));
            return;
        },
    }

    match parent_job.finished_children.lock() {
        Ok(mut finished) => {
            finished.push(File::Stub(StubInformation {
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