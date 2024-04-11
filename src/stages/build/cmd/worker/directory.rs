use std::fs;
use std::fs::DirEntry;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use log::{error, trace};
use crate::file::{DirectoryInformation, File};
use crate::hash::GeneralHash;
use crate::stages::build::cmd::job::{BuildJob, BuildJobState, JobResult};
use crate::stages::build::cmd::worker::{worker_create_error, worker_fetch_savedata, worker_publish_result_or_trigger_parent, WorkerArgument};
use crate::stages::build::output::HashTreeFileEntryType;

/// Analyze a directory.
/// 
/// # Arguments
/// * `path` - The path to the directory.
/// * `modified` - The last modified time of the directory.
/// * `size` - The size of the directory (given by fs::metadata).
/// * `id` - The id of the worker.
/// * `job` - The job to process.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
pub fn worker_run_directory(path: PathBuf, modified: u64, size: u64, id: usize, mut job: BuildJob, result_publish: &Sender<JobResult>, job_publish: &Sender<BuildJob>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing directory {} > {:?}", id, &job.target_path, path);

    match job.state {
        BuildJobState::NotProcessed => {
            let read_dir = fs::read_dir(&path);
            let read_dir = match read_dir {
                Ok(read_dir) => read_dir,
                Err(err) => {
                    error!("Error while reading directory {:?}: {}", path, err);
                    worker_publish_result_or_trigger_parent(id, false, worker_create_error(job.target_path.clone(), modified, size), job, result_publish, job_publish, arg);
                    return;
                }
            };
            let mut read_dir: Vec<DirEntry> = read_dir
                .filter_map(|entry| {
                    match entry {
                        Ok(entry) => {
                            Some(entry)
                        },
                        Err(err) => {
                            error!("Error while reading directory entry {:?}: {}", path, err);
                            None
                        }
                    }
                }).collect();
            read_dir.sort_by_key(|entry| entry.file_name());

            let mut children = Vec::new();

            for entry in read_dir {
                let child_path = job.target_path.child(entry.file_name());
                children.push(child_path);
            }

            job.state = BuildJobState::Analyzed;

            let parent_job = Arc::new(job);
            let mut jobs = Vec::with_capacity(children.len());

            for child in children {
                let job = BuildJob::new(Some(Arc::clone(&parent_job)), child);
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
        BuildJobState::Analyzed => {
            let mut hash = GeneralHash::from_type(arg.hash_type);
            let mut children = Vec::new();

            let mut cached_entry = None;
            let mut error;
            match job.finished_children.lock() {
                Ok(mut finished) => {
                    finished.sort_by(|a, b| a.get_content_hash().partial_cmp(b.get_content_hash()).expect("Two hashes must compare to each other"));

                    error = false;
                    
                    // query cache
                    match worker_fetch_savedata(arg, &job.target_path) {
                        Some(found) => {
                            if found.file_type == HashTreeFileEntryType::Directory && found.modified == modified && found.size == finished.len() as u64 {
                                if found.children.len() == finished.len() && found.children.iter().zip(finished.iter().map(|e| e.get_content_hash())).all(|(a, b)| a == b) {
                                    trace!("Directory {:?} is already in save file", path);

                                    let mut children = Vec::new();
                                    children.append(finished.deref_mut());

                                    let file = File::Directory(DirectoryInformation {
                                        path: job.target_path.clone(),
                                        modified,
                                        content_hash: found.hash.clone(),
                                        number_of_children: children.len() as u64,
                                        children,
                                    });

                                    cached_entry = Some(file);
                                }
                            }
                        }
                        None => {}
                    }

                    if cached_entry.is_none() {
                        match hash.hash_directory(finished.iter()) {
                            Ok(_) => {},
                            Err(err) => {
                                error = true;
                                error!("Error while hashing directory {:?}: {}", path, err);
                            }
                        }
                        children.append(finished.deref_mut());
                    }
                }
                Err(err) => {
                    error!("[{}] failed to lock finished children: {}", id, err);
                    error = true;
                }
            }
            if error {
                worker_publish_result_or_trigger_parent(id, false, worker_create_error(job.target_path.clone(), modified, size), job, result_publish, job_publish, arg);
                return;
            }

            if let Some(file) = cached_entry {
                worker_publish_result_or_trigger_parent(id, true, file, job, result_publish, job_publish, arg);
                return;
            }

            let file = File::Directory(DirectoryInformation {
                path: job.target_path.clone(),
                modified,
                content_hash: hash,
                number_of_children: children.len() as u64,
                children,
            });

            worker_publish_result_or_trigger_parent(id, false, file, job, result_publish, job_publish, arg);
        }
    }
}