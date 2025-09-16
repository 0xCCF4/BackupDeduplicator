use crate::hash::GeneralHash;
use crate::pool::ThreadPool;
use crate::shallow_ref_tree;
use crate::shallow_ref_tree::{NodeId, ShallowRefTree};
use crate::stages::build::cmd::worker::{worker_run, WorkerArgument};
use crate::stages::build::cmd::{
    BuildJob, BuildJobData, BuildSettings, FileType, JobResult, JobResultData,
};
use log::{error, info, trace, warn};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

#[derive(Debug)]
enum JobTreeData {
    Root,
    NotScheduled(BuildJobData),

    BlockedByChildren(BuildJobData),
    Running(BuildJobData),

    Finished(JobResultData),
}

impl JobTreeData {
    pub fn mark_running(&mut self) {
        if let JobTreeData::BlockedByChildren(job) | JobTreeData::NotScheduled(job) = self {
            *self = JobTreeData::Running(job.clone());
        }
    }
    pub fn path(&self) -> Option<&PathBuf> {
        self.job().map(|x|x.path()).or(self.result().map(|x|x.path()))
    }
    pub fn job(&self) -> Option<&BuildJobData> {
        match self {
            JobTreeData::BlockedByChildren(job)
            | JobTreeData::Running(job)
            | JobTreeData::NotScheduled(job) => Some(job),
            JobTreeData::Root | JobTreeData::Finished(..) => None,
        }
    }
    pub fn job_mut(&mut self) -> Option<&mut BuildJobData> {
        match self {
            JobTreeData::BlockedByChildren(job)
            | JobTreeData::Running(job)
            | JobTreeData::NotScheduled(job) => Some(job),
            JobTreeData::Root | JobTreeData::Finished(..) => None,
        }
    }
    pub fn result(&self) -> Option<&JobResultData> {
        match self {
            JobTreeData::Finished(result) => Some(result),
            JobTreeData::Running(_)
            | JobTreeData::BlockedByChildren(_)
            | JobTreeData::Root
            | JobTreeData::NotScheduled(_) => None,
        }
    }
    pub fn is_finished(&self) -> bool {
        matches!(self, JobTreeData::Finished(_))
    }
}

pub struct JobPlanner {
    tree: ShallowRefTree<JobTreeData>,
    scheduled_jobs: BTreeSet<shallow_ref_tree::NodeId>,
    running_jobs: BTreeMap<u64, shallow_ref_tree::NodeId>,

    pool: ThreadPool<BuildJob, JobResult>,
}

pub enum ScheduleJobsResult {
    Finished,
    Error {
        path: PathBuf,
        reason: String,
    },
    Ok,
}

impl JobPlanner {
    pub fn new(number_of_threads: usize, settings: &BuildSettings) -> Self {
        let mut args = Vec::with_capacity(number_of_threads);
        for _ in 0..args.capacity() {
            args.push(WorkerArgument {
                archives: settings.into_archives,
                follow_symlinks: settings.follow_symlinks,
                hash_type: settings.hash_type,
            });
        }

        let tree = ShallowRefTree::new(JobTreeData::Root).into();

        Self {
            running_jobs: BTreeMap::new(),
            tree,
            scheduled_jobs: BTreeSet::new(),

            pool: ThreadPool::new(args, worker_run),
        }
    }

    pub fn schedule_initial_job(&mut self, path: PathBuf) -> Option<NodeId> {
        self.schedule_child(self.tree.root_id, BuildJobData::Initial(path))
    }

    pub fn schedule_child(&mut self, parent: NodeId, job: BuildJobData) -> Option<NodeId> {
        if let Some(node) = self.tree.add_child(parent, JobTreeData::NotScheduled(job)) {
            self.scheduled_jobs.insert(node);
            Some(node)
        } else {
            None
        }
    }

    pub fn process_result(&mut self, result: JobResult) {
        match self.running_jobs.remove(&result.job_id) {
            Some(job) => job,
            None => {
                error!("Received result for unknown job: {}", result.job_id);
                return;
            }
        };

        trace!("[RECEIVE] Processing job results: {result:?}");

        match result.result {
            x @ JobResultData::ArchiveHash { .. }
            | x @ JobResultData::SymlinkHash { .. }
            | x @ JobResultData::DirectoryHash { .. }
            | x @ JobResultData::FileHash { .. }
            | x @ JobResultData::Other { .. }
            | x @ JobResultData::Error { .. } => {
                let node = self.tree.node_mut(result.node_id).expect("node must exist");
                node.content = JobTreeData::Finished(x);
                trace!("- mark finished");

                if let Some(parent) = self.tree.parent_ref(result.node_id) {
                    if !parent.content.is_finished() {
                        self.scheduled_jobs.insert(parent.id());
                    }
                    trace!("- schedule parent {}", parent.id());
                }
            }
            JobResultData::DirectoryListing { path, children } => {
                let mut blocking = false;
                for entry in children {
                    match entry.file_type {
                        FileType::Directory => {
                            self.schedule_child(
                                result.node_id,
                                BuildJobData::DiscoverDirectory(entry.path),
                            );
                            blocking = true;
                        }
                        FileType::File => {
                            self.schedule_child(result.node_id, BuildJobData::HashFile(entry.path));
                            blocking = true;
                        }
                        FileType::Symlink => {
                            self.schedule_child(
                                result.node_id,
                                BuildJobData::HashSymlink(entry.path),
                            );
                            blocking = true;
                        }
                        FileType::Other => {
                            let node = self.tree.node_mut(result.node_id).expect("node must exist");
                            node.content = JobTreeData::Finished(JobResultData::Other {
                                path: entry.path,
                                hash: GeneralHash::NULL,
                            });
                        }
                    }
                }

                let node = self.tree.node_mut(result.node_id).expect("node must exist");
                node.content = JobTreeData::BlockedByChildren(BuildJobData::DirectoryStub(path));

                trace!("- set blocked");

                if !blocking {
                    self.scheduled_jobs.insert(result.node_id);
                    trace!("- schedule node");
                }
            }
            JobResultData::InitialEvaluation(entry) => {
                let node = self.tree.node_mut(result.node_id).expect("node must exist");
                match entry.file_type {
                    FileType::Directory => {
                        node.content =
                            JobTreeData::NotScheduled(BuildJobData::DiscoverDirectory(entry.path));
                        self.scheduled_jobs.insert(result.node_id);
                    }
                    FileType::File => {
                        node.content =
                            JobTreeData::NotScheduled(BuildJobData::HashFile(entry.path));
                        self.scheduled_jobs.insert(result.node_id);
                    }
                    FileType::Symlink => {
                        node.content =
                            JobTreeData::NotScheduled(BuildJobData::HashSymlink(entry.path));
                        self.scheduled_jobs.insert(result.node_id);
                    }
                    FileType::Other => {
                        node.content = JobTreeData::Finished(JobResultData::Other {
                            path: entry.path,
                            hash: GeneralHash::NULL,
                        });
                        self.scheduled_jobs.insert(self.tree.root_id);
                    }
                }
            }
        }
    }

    fn schedule_jobs(&mut self) -> ScheduleJobsResult {
        let jobs = std::mem::take(&mut self.scheduled_jobs);

        for node_id in jobs {
            let node = self.tree.node(node_id);
            if let Some(node) = node {
                trace!("[SCHEDULE] {:?}", node);
                match &node.content {
                    JobTreeData::BlockedByChildren { .. } => match self.tree.children_ref(node) {
                        None => {
                            warn!("Encountered BLOCKED-BY-CHILDREN while no children were found")
                        }
                        Some(children) => {
                            if children.iter().any(|child| matches!(child.content, JobTreeData::Finished(JobResultData::Error {..}))) || children
                                .iter()
                                .all(|child| matches!(child.content, JobTreeData::Finished { .. }))
                            {
                                trace!("- all children evaluated or error");
                                let children_nodes =
                                    children.iter().map(|x| x.content.result());

                                let mut hashes = Vec::new();
                                let mut error_found = None;
                                for child in children_nodes.flatten() {

                                        match child {
                                            JobResultData::ArchiveHash { file_hash, .. }
                                            | JobResultData::SymlinkHash {
                                                hash: file_hash, ..
                                            }
                                            | JobResultData::DirectoryHash {
                                                hash: file_hash, ..
                                            }
                                            | JobResultData::FileHash {
                                                hash: file_hash, ..
                                            }
                                            | JobResultData::Other {
                                                hash: file_hash, ..
                                            } => {
                                                hashes.push(file_hash);
                                            }
                                            JobResultData::Error { path, reason } => {
                                                error_found = Some((path.clone(), reason.clone()));
                                                break;
                                            }
                                            JobResultData::DirectoryListing { path, children: _ } => {
                                                error_found = Some((path.clone(), "Parent was evaluated and child did not hash properly".to_string()));
                                                break;
                                            }
                                            JobResultData::InitialEvaluation(entry) => {
                                                error_found = Some((entry.path.clone(), "Parent was evaluated and child was of initial eval type".to_string()));
                                                break;
                                            }
                                        }
                                }

                                if let Some((path, reason)) = error_found {
                                    trace!("- bubble up error");
                                    let mut current = Some(node_id);
                                    while let Some(node) = current {
                                        trace!("- setting: {node} to error");
                                        let parent = self.tree.node_mut(node_id).unwrap();
                                        if !matches!(parent.content, JobTreeData::Root) {
                                            parent.content = JobTreeData::Finished(JobResultData::Error {
                                                path: path.clone(),
                                                reason: reason.clone(),
                                            });
                                        };
                                        current = self.tree.parent(node);
                                    }
                                    self.scheduled_jobs.insert(self.tree.root_id);
                                    continue;
                                }

                                let parent = self.tree.parent_ref(node_id).expect("parent must exist");
                                if let Some(parent_path) = parent.content.path() { // not some for Root
                                    trace!("- prepare parent dire hash");
                                    let directory_job = BuildJobData::HashDirectory {
                                        path: parent_path.clone(),
                                        children: hashes.into_iter().cloned().collect(),
                                    };
                                    let parent = self.tree.parent_mut(node_id).expect("parent must exist");
                                    parent.content = JobTreeData::Running(directory_job.clone());

                                    trace!("- publish job");
                                    let job = BuildJob::new(directory_job, parent.id());
                                    let job_id = job.job_id;
                                    if self.pool.publish_if_alive(job) {
                                        self.running_jobs.insert(job_id, parent.id());
                                    }
                                }
                            }
                        }
                    },
                    JobTreeData::NotScheduled { .. } => {
                        let node = self.tree.node_mut(node_id).unwrap();
                        trace!("Mark as running");
                        node.content.mark_running();

                        let job = BuildJob::new(node.content.job().unwrap().clone(), node_id);
                        trace!("[DISPATCH] Starting job: {job:?}");
                        let job_id = job.job_id;
                        if self.pool.publish_if_alive(job) {
                            self.running_jobs.insert(job_id, node_id);
                        }
                    }
                    JobTreeData::Running { .. } => {
                        warn!("Encountered RUNNING job while scheduling jobs");
                    }
                    JobTreeData::Finished { .. } => {
                        warn!("Encountered FINISHED job while scheduling jobs");
                    }
                    JobTreeData::Root => {
                        trace!("- encountered root, checking exit condition");
                        let immediate_children = self.tree.children_ref(self.tree.root_id).unwrap();
                        let mut finished = true;
                        for child in immediate_children {
                            match &child.content {
                                JobTreeData::Finished(JobResultData::Error { path, reason }) => {
                                    trace!("- error");
                                    return ScheduleJobsResult::Error {path: path.clone(), reason: reason.clone()};
                                }
                                JobTreeData::Finished(_) => {}
                                _ => {
                                    finished = false;
                                    break;
                                }
                            }
                        }
                        if finished {
                            return ScheduleJobsResult::Finished
                        }
                    }
                }
            }
        }
        ScheduleJobsResult::Ok
    }

    pub fn run(&mut self) -> Result<(), (PathBuf, String)> {
        loop {
            let mut stopping_with = None;
            match self.schedule_jobs() {
                ScheduleJobsResult::Ok => {},
                ScheduleJobsResult::Error { path, reason } => {
                    error!("Stopped job dispatcher because of error with path {:?}: {}", path, reason);
                    stopping_with = Some(Err((path, reason)));
                },
                ScheduleJobsResult::Finished => {
                    trace!("Gracefully stopping");
                    stopping_with = Some(Ok(()));
                }
            }

            if let Some(stopping_with) = stopping_with {
                info!("Letting all running jobs stop gracefully");
                self.pool.close();
                while !self.running_jobs.is_empty() {
                    while let Ok(result) = self.pool.receive() {
                        self.process_result(result);
                    }
                }
                return stopping_with;
            }

            match self.pool.receive() {
                Ok(result) => {
                    self.process_result(result);
                }
                Err(_) => {
                    error!("Worker threads have exited. Exiting planner.");
                    return Err(("".into(), "Worker threads exited prematurely!".into()));
                }
            }
        }
    }
}
