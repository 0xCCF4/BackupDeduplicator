use crate::hash::GeneralHash;
use crate::pool::ThreadPool;
use crate::shallow_ref_tree;
use crate::shallow_ref_tree::{NodeId, ShallowRefTree};
use crate::stages::build::cmd::worker::{worker_run, WorkerArgument};
use crate::stages::build::cmd::{
    BuildJob, BuildJobData, BuildSettings, FileType, JobResult, JobResultData,
};
use log::{error, warn};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

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
    /*pub fn path(&self) -> &PathBuf {
        match self {
            JobTreeData::BlockedByChildren(job)
            | JobTreeData::Running(job)
            | JobTreeData::NotScheduled(job) => job.path(),
            JobTreeData::Root => &PathBuf::new(),
            JobTreeData::Finished(JobResultData::ArchiveHash {path, ..}) |
            JobTreeData::Finished(JobResultData::SymlinkHash {path, ..}) |
            JobTreeData::Finished(JobResultData::DirectoryHash {path, ..}) |
            JobTreeData::Finished(JobResultData::FileHash {path, ..}) |
            JobTreeData::Finished(JobResultData::Other {path, ..}) |
            JobTreeData::Finished(JobResultData::Error) => path,
            JobTreeData::Finished(JobResultData::DirectoryListing {path, ..}) => path,
            JobTreeData::Finished(JobResultData::InitialEvaluation(entry)) => &entry.path,
        }
    }*/
}

pub struct JobPlanner {
    tree: ShallowRefTree<JobTreeData>,
    scheduled_jobs: Vec<shallow_ref_tree::NodeId>,
    running_jobs: BTreeMap<u64, shallow_ref_tree::NodeId>,

    pool: ThreadPool<BuildJob, JobResult>,
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
            scheduled_jobs: Vec::new(),

            pool: ThreadPool::new(args, worker_run),
        }
    }

    pub fn schedule_initial_job(&mut self, path: PathBuf) -> Option<NodeId> {
        self.schedule_child(self.tree.root_id, BuildJobData::Initial(path))
    }

    pub fn schedule_child(&mut self, parent: NodeId, job: BuildJobData) -> Option<NodeId> {
        if let Some(node) = self.tree.add_child(parent, JobTreeData::NotScheduled(job)) {
            self.scheduled_jobs.push(node);
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

        match result.result {
            x @ JobResultData::ArchiveHash { .. }
            | x @ JobResultData::SymlinkHash { .. }
            | x @ JobResultData::DirectoryHash { .. }
            | x @ JobResultData::FileHash { .. }
            | x @ JobResultData::Other { .. }
            | x @ JobResultData::Error { .. } => {
                let node = self.tree.node_mut(result.node_id).expect("node must exist");
                node.content = JobTreeData::Finished(x);

                if let Some(parent) = self.tree.parent(result.node_id) {
                    self.scheduled_jobs.push(parent);
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

                if !blocking {
                    self.scheduled_jobs.push(result.node_id);
                }
            }
            JobResultData::InitialEvaluation(entry) => {
                let node = self.tree.node_mut(result.node_id).expect("node must exist");
                match entry.file_type {
                    FileType::Directory => {
                        node.content =
                            JobTreeData::NotScheduled(BuildJobData::DiscoverDirectory(entry.path));
                        self.scheduled_jobs.push(result.node_id);
                    }
                    FileType::File => {
                        node.content =
                            JobTreeData::NotScheduled(BuildJobData::HashFile(entry.path));
                        self.scheduled_jobs.push(result.node_id);
                    }
                    FileType::Symlink => {
                        node.content =
                            JobTreeData::NotScheduled(BuildJobData::HashSymlink(entry.path));
                        self.scheduled_jobs.push(result.node_id);
                    }
                    FileType::Other => {
                        node.content = JobTreeData::Finished(JobResultData::Other {
                            path: entry.path,
                            hash: GeneralHash::NULL,
                        });
                        self.scheduled_jobs.push(self.tree.root_id);
                    }
                }
            }
        }
    }

    fn schedule_jobs(&mut self) {
        let jobs = std::mem::take(&mut self.scheduled_jobs);

        for node_id in jobs {
            let node = self.tree.node(node_id);
            if let Some(node) = node {
                match &node.content {
                    JobTreeData::BlockedByChildren { .. } => match self.tree.children_ref(node) {
                        None => {
                            warn!("Encountered BLOCKED-BY-CHILDREN while no children were found")
                        }
                        Some(children) => {
                            if children
                                .iter()
                                .all(|child| matches!(child.content, JobTreeData::Finished { .. }))
                            {
                                let children_nodes =
                                    children.iter().map(|x| x.content.result().unwrap());

                                let hashes = Vec::new();
                                let mut error_found = false;
                                for child in children_nodes {
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
                                            // todo
                                            // TODO: CONTINUE HERE
                                        }
                                    }
                                }

                                let directory_job = BuildJobData::HashDirectory {
                                    children: children_nodes.map(|child| child.result().unwrap()),
                                };

                                let node = self.tree.node_mut(node_id).unwrap();
                                // todo put directory references

                                if let Some(job) = node.content.job_mut() {}

                                node.content.mark_running();

                                let job =
                                    BuildJob::new(node.content.job().unwrap().clone(), node_id);
                                self.running_jobs.insert(job.job_id, node_id);
                                self.pool.publish(job);
                            }
                        }
                    },
                    JobTreeData::NotScheduled { .. } => {
                        let node = self.tree.node_mut(node_id).unwrap();
                        node.content.mark_running();

                        let job = BuildJob::new(node.content.job().unwrap().clone(), node_id);
                        self.running_jobs.insert(job.job_id, node_id);
                        self.pool.publish(job);

                        self.scheduled_jobs.retain(|id| *id != node_id);
                    }
                    JobTreeData::Running { .. } => {
                        warn!("Encountered RUNNING job while scheduling jobs");
                    }
                    JobTreeData::Finished { .. } => {
                        warn!("Encountered FINISHED job while scheduling jobs");
                    }
                    JobTreeData::Root => {
                        warn!("Encountered ROOT job while scheduling jobs");
                    }
                }
            }
        }
    }

    pub fn open_jobs(&mut self) -> usize {
        self.running_jobs.len() + self.scheduled_jobs.len()
    }

    pub fn run(&mut self) {
        while self.open_jobs() > 0 {
            self.schedule_jobs();

            match self.pool.receive_timeout(Duration::from_millis(100)) {
                Ok(result) => {
                    self.process_result(result);
                }
                Err(RecvTimeoutError::Timeout) => {
                    // just continue
                }
                Err(RecvTimeoutError::Disconnected) => {
                    error!("Worker threads have exited. Exiting planner.");
                    return;
                }
            }
        }
    }
}
