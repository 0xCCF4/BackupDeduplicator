use std::sync::{Arc, Mutex};
use crate::data::{File, FilePath};

pub type SharedJob = Arc<Job>;

static JOB_COUNTER: Mutex<usize> = Mutex::new(0);

fn new_job_counter_id() -> usize {
    let mut counter = JOB_COUNTER.lock().expect("Failed to lock job counter");
    *counter += 1;
    (*counter).clone()
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum JobState {
    NotProcessed,
    Analyzed,
}

#[derive(Debug)]
pub struct Job {
    id: usize,
    pub parent: Option<SharedJob>,
    pub finished_children: Mutex<Vec<File>>,
    pub target_path: FilePath,
    pub state: JobState,
}

impl Job {
    pub fn new(parent: Option<SharedJob>, target_path: FilePath) -> Self {
        Job {
            id: new_job_counter_id(),
            parent,
            target_path,
            state: JobState::NotProcessed,
            finished_children: Mutex::new(Vec::new()),
        }
    }
    
    pub fn job_id(&self) -> usize {
        self.id
    }

    pub(crate) fn new_job_id(mut self) -> Self {
        self.id = new_job_counter_id();
        self
    }
}

impl JobTrait for Job {
    fn job_id(&self) -> usize {
        Job::job_id(self)
    }
}


pub trait JobTrait<T: std::marker::Send = Self> {
    fn job_id(&self) -> usize;
}

pub trait ResultTrait<T: std::marker::Send = Self> {}
