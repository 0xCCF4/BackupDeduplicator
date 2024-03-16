use std::fs;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use log::{error, trace};
use crate::build::JobResult;
use crate::build::worker::{worker_create_error, worker_publish_result_or_trigger_parent, WorkerArgument};
use crate::data::{DirectoryInformation, File, GeneralHash, Job, JobState};
use crate::utils;

pub fn worker_run_directory(path: PathBuf, modified: u64, id: usize, mut job: Job, result_publish: &Sender<JobResult>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing directory {:?}#{:?}", id, &job.target_path, path);
    match job.state {
        JobState::NotProcessed => {
            let read_dir = fs::read_dir(&path);
            let read_dir = match read_dir {
                Ok(read_dir) => read_dir,
                Err(err) => {
                    error!("Error while reading directory {:?}: {}", path, err);
                    worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
                    return;
                }
            };

            let mut children = Vec::new();

            for entry in read_dir {
                match entry {
                    Ok(entry) => {
                        let child_path = job.target_path.child_real(entry.file_name());
                        children.push(child_path);
                    },
                    Err(err) => {
                        error!("Error while reading directory entry {:?}: {}", path, err);
                    }
                };
            }

            job.state = JobState::Analyzed;

            let parent_job = Arc::new(job);
            let mut jobs = Vec::with_capacity(children.len());

            for child in children {
                let job = Job::new(Some(Arc::clone(&parent_job)), child);
                jobs.push(job);
            }

            drop(parent_job);

            for job in jobs {
                match job_publish.send(job) {
                    Ok(_) => {},
                    Err(e) => {
                        error!("[{}] failed to publish job: {}", id, e);
                    }
                }
            }
        },
        JobState::Analyzed => {
            let mut hash = GeneralHash::from_type(arg.hash);
            let mut children = Vec::new();

            let mut error;
            match job.finished_children.lock() {
                Ok(mut finished) => {
                    error = false;
                    match utils::hash_directory(finished.iter(), &mut hash) {
                        Ok(_) => {},
                        Err(err) => {
                            error = true;
                            error!("Error while hashing directory {:?}: {}", path, err);
                        }
                    }
                    children.append(finished.deref_mut());
                }
                Err(err) => {
                    error!("[{}] failed to lock finished children: {}", id, err);
                    error = true;
                }
            }
            if error {
                worker_publish_result_or_trigger_parent(id, worker_create_error(job.target_path.clone()), job, result_publish, job_publish, arg);
                return;
            }

            let file = File::Directory(DirectoryInformation {
                path: job.target_path.clone(),
                modified,
                content_hash: hash,
                number_of_children: children.len() as u64,
                children,
            });

            worker_publish_result_or_trigger_parent(id, file, job, result_publish, job_publish, arg);
        }
    }
}