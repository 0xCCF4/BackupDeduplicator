use std::path::PathBuf;
use std::sync::mpsc::Sender;
use log::trace;
use crate::build::JobResult;
use crate::build::worker::{worker_fetch_savedata, worker_publish_result_or_trigger_parent, WorkerArgument};
use crate::data::{File, Job, OtherInformation, SaveFileEntryType};

pub fn worker_run_other(path: PathBuf, modified: u64, size: u64, id: usize, job: Job, result_publish: &Sender<JobResult>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing other {} > {:?}", id, &job.target_path, path);

    match worker_fetch_savedata(arg, &job.target_path) {
        Some(found) => {
            if found.file_type == SaveFileEntryType::Other && found.modified == modified && found.size == size {
                trace!("Other {:?} is already in save file", path);
                worker_publish_result_or_trigger_parent(id, true, File::Other(OtherInformation {
                    path: job.target_path.clone(),
                    content_size: size,
                    modified,
                }), job, result_publish, job_publish, arg);
                return;
            }
        }
        None => {}
    }
    
    let file = File::Other(OtherInformation {
        path: job.target_path.clone(),
        content_size: size,
        modified,
    });

    worker_publish_result_or_trigger_parent(id, false, file, job, result_publish, job_publish, arg);
}