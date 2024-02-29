use std::sync::{Arc, Mutex};
use crate::data::FilePath;

pub type SharedJob = Arc<Mutex<Job>>;

static JOB_COUNTER: Mutex<usize> = Mutex::new(0);

fn new_job_counter_id() -> usize {
    let mut counter = JOB_COUNTER.lock().expect("Failed to lock job counter");
    *counter += 1;
    (*counter).clone()
}

#[derive(Debug)]
pub struct Job {
    id: usize,
    pub parent: Option<SharedJob>,
    pub unfinished_children: Mutex<u32>,
    pub target_path: FilePath,
}

impl Job {
    pub fn new(parent: Option<SharedJob>, target_path: FilePath) -> Self {
        Job {
            id: new_job_counter_id(),
            parent,
            unfinished_children: Mutex::new(0),
            target_path,
        }
    }
    
    pub fn job_id(&self) -> usize {
        self.id
    }
}
