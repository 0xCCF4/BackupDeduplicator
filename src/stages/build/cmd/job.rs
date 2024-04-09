use std::sync::{Arc, Mutex};
use serde::Serialize;
use crate::file::File;
use crate::path::FilePath;
use crate::pool::{JobTrait, ResultTrait};

pub type SharedBuildJob = Arc<BuildJob>;

static JOB_COUNTER: Mutex<usize> = Mutex::new(0);

fn new_job_counter_id() -> usize {
    let mut counter = JOB_COUNTER.lock().expect("Failed to lock job counter");
    *counter += 1;
    (*counter).clone()
}

/// The state of a build job. Used to track the state of a directory process job.
/// 
/// # Fields
/// * `NotProcessed` - The job has not been processed yet.
/// * `Analyzed` - The directory has been expanded and can be analyzed further.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum BuildJobState {
    NotProcessed,
    Analyzed,
}

/// A build job. Used to issue a job to hash a file/directory.
/// 
/// # Fields
/// * `parent` - The parent job of this job.
/// * `finished_children` - The finished children of this job.
/// * `target_path` - The path of the file/directory to hash.
/// * `state` - The state of the job.
#[derive(Debug)]
pub struct BuildJob {
    id: usize,
    pub parent: Option<SharedBuildJob>,
    pub finished_children: Mutex<Vec<File>>,
    pub target_path: FilePath,
    pub state: BuildJobState,
}

impl BuildJob {
    /// Create a new build job.
    /// 
    /// # Arguments
    /// * `parent` - The parent job of this job.
    /// * `target_path` - The path of the file/directory to hash.
    /// 
    /// # Returns
    /// The created build job.
    pub fn new(parent: Option<SharedBuildJob>, target_path: FilePath) -> Self {
        BuildJob {
            id: new_job_counter_id(),
            parent,
            target_path,
            state: BuildJobState::NotProcessed,
            finished_children: Mutex::new(Vec::new()),
        }
    }
    
    /// Get the job id.
    /// 
    /// # Returns
    /// The job id.
    pub fn job_id(&self) -> usize {
        self.id
    }

    /// Create and assign a new unique job id.
    /// 
    /// # Returns
    /// The build job with the new job id.
    pub fn new_job_id(mut self) -> Self {
        self.id = new_job_counter_id();
        self
    }
}

impl JobTrait for BuildJob {
    /// Get the job id.
    /// 
    /// # Returns
    /// The job id.
    fn job_id(&self) -> usize {
        BuildJob::job_id(self)
    }
}

/// The result of a build job.
///
/// # Fields
/// * `already_cached` - Whether the content was already cached.
/// * `content` - The content of the job result.
#[derive(Debug, Serialize, Clone)]
pub struct JobResultContent {
    pub already_cached: bool,
    pub content: File,
}

/// A job result.
/// 
/// # Fields
/// * `Final` - The final result of command. Returned if the job has no parent.
/// * `Intermediate` - An intermediate result of a command. Returned if the job has a parent.
#[derive(Debug, Serialize, Clone)]
pub enum JobResult {
    Final(JobResultContent),
    Intermediate(JobResultContent),
}

impl ResultTrait for JobResult {

}

