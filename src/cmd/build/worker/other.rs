use std::path::PathBuf;
use std::sync::mpsc::Sender;
use log::trace;
use crate::build::JobResult;
use crate::build::worker::{worker_publish_result_or_trigger_parent, WorkerArgument};
use crate::data::{File, Job, OtherInformation};

pub fn worker_run_other(path: PathBuf, _modified: u64, id: usize, job: Job, result_publish: &Sender<JobResult>, job_publish: &Sender<Job>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing other {:?}#{:?}", id, &job.target_path, path);
    let file = File::Other(OtherInformation {
        path: job.target_path.clone(),
    });

    worker_publish_result_or_trigger_parent(id, file, job, result_publish, job_publish, arg);
}