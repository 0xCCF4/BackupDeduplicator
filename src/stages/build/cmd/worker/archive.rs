use std::path::PathBuf;
use std::sync::mpsc::Sender;
use crate::stages::build::cmd::job::{BuildJob, JobResult};
use crate::stages::build::cmd::worker::WorkerArgument;

pub fn worker_run_file(path: PathBuf, modified: u64, size: u64, id: usize, job: BuildJob, result_publish: &Sender<JobResult>, job_publish: &Sender<BuildJob>, arg: &mut WorkerArgument) {
    todo!("worker_run_file")
}
