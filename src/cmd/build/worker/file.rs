use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use log::{error, trace};
use crate::build::JobResult;
use crate::build::worker::{worker_create_error, worker_publish_result_or_trigger_parent, WorkerArgument};
use crate::data::{GeneralHash, Job, GeneralHashType, File, FileInformation};
use crate::utils;

pub fn worker_run_file(path: PathBuf, modified: u64, id: usize, job: Job, result_publish: &Sender<JobResult>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing file {:?}#{:?}", id, &job.target_path, path);
    match fs::File::open(&path) {
        Ok(file) => {
            let mut reader = std::io::BufReader::new(file);
            let mut hash = GeneralHash::from_type(arg.hash_type);
            let content_size;

            if arg.hash_type == GeneralHashType::NULL {
                // dont hash file
                content_size = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
            } else {
                match utils::hash_file(&mut reader, &mut hash) {
                    Ok(size) => {
                        content_size = size;
                    }
                    Err(err) => {
                        error!("Error while hashing file {:?}: {}", path, err);
                        worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
                        return;
                    }
                }
            }

            let file = File::File(FileInformation {
                path: job.target_path.clone(),
                modified,
                content_hash: hash,
                content_size,
            });
            worker_publish_result_or_trigger_parent(id, file, job, result_publish, job_publish, arg);
            return;
        }
        Err(err) => {
            error!("Error while opening file {:?}: {}", path, err);
            worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
            return;
        }
    }
}