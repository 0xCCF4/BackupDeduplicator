use crate::hash::GeneralHash;
use crate::path::FilePath;
use crate::pool::ThreadPool;
use crate::shallow_ref_tree;
use crate::shallow_ref_tree::{DebugGraph, NodeId, ShallowRefTree};
use crate::stages::build::cmd::worker::{worker_run, WorkerArgument};
use crate::stages::build::cmd::{
    BuildJob, BuildJobData, BuildSettings, DirectoryEntry, FileType, JobResult, JobResultData,
};
use crate::stages::build::output::{HashTreeFileEntry, HashTreeFileEntryType};
use itertools::Itertools;
use log::{error, info, trace, warn};
use std::collections::{hash_map, BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;

#[derive(Debug)]
enum JobTreeData {
    Root,
    NotScheduled(BuildJobData),

    BlockedByChildren(BuildJobData),
    Running(BuildJobData),

    Finished(JobResultData),
}

impl DebugGraph for JobTreeData {
    type Color = &'static str;
    type Label = String;
    fn debug_color(&self) -> Option<Self::Color> {
        match self {
            JobTreeData::Root => None,
            JobTreeData::NotScheduled(_) => Some("gray"),
            JobTreeData::BlockedByChildren(_) => Some("yellow"),
            JobTreeData::Running(_) => Some("cyan"),
            JobTreeData::Finished(JobResultData::Error { .. }) => Some("red"),
            JobTreeData::Finished(_) => Some("green"),
        }
    }
    fn label(&self) -> Option<Self::Label> {
        if let JobTreeData::Root = self {
            return Some("Root".into());
        }

        self.path().map(|x| x.to_string_lossy().into())
    }
}

impl JobTreeData {
    pub fn mark_running(&mut self) {
        if let JobTreeData::BlockedByChildren(job) | JobTreeData::NotScheduled(job) = self {
            *self = JobTreeData::Running(job.clone());
        }
    }
    pub fn path(&self) -> Option<&PathBuf> {
        self.job()
            .map(|x| x.path())
            .or(self.result().map(|x| x.path()))
    }
    pub fn entry(&self) -> Option<&DirectoryEntry> {
        self.job()
            .and_then(|x| x.entry())
            .or(self.result().and_then(|x| x.entry()))
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

pub struct JobPlanner<'a> {
    tree: ShallowRefTree<JobTreeData>,
    scheduled_jobs: BTreeSet<shallow_ref_tree::NodeId>,
    running_jobs: BTreeMap<u64, shallow_ref_tree::NodeId>,

    pool: ThreadPool<BuildJob, JobResult>,

    cache: &'a mut HashMap<FilePath, HashTreeFileEntry>,
    result_sender: Sender<HashTreeFileEntry>,

    _debug_dotfiles: AtomicU32,
}

pub enum ScheduleJobsResult {
    Finished,
    Error {
        occurred_at: PathBuf,
        reason: String,
    },
    Ok,
}

impl<'a> JobPlanner<'a> {
    pub fn new(
        number_of_threads: usize,
        settings: &BuildSettings,
        cache: &'a mut HashMap<FilePath, HashTreeFileEntry>,
        result_sender: Sender<HashTreeFileEntry>,
    ) -> Self {
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

            cache,
            result_sender,

            pool: ThreadPool::new(args, worker_run),
            _debug_dotfiles: 0.into(),
        }
    }

    pub fn schedule_initial_job(&mut self, path: PathBuf) -> Option<NodeId> {
        self.schedule_child(self.tree.root_id(), BuildJobData::Initial(path))
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
        //self.debug_print_graph();

        match self.running_jobs.remove(&result.job_id) {
            Some(job) => job,
            None => {
                error!("Received result for unknown job: {}", result.job_id);
                return;
            }
        };

        trace!("[RECEIVE] Processing job results: {result:?}");

        match result.result {
            mut x @ JobResultData::ArchiveHash { .. }
            | mut x @ JobResultData::CachedArchiveHash { .. }
            | mut x @ JobResultData::SymlinkHash { .. }
            | mut x @ JobResultData::DirectoryHash { .. }
            | mut x @ JobResultData::FileHash { .. }
            | mut x @ JobResultData::Other { .. }
            | mut x @ JobResultData::Error { .. } => {
                for data in x.hash_tree_file_entry().into_iter() {
                    let _ = self.result_sender.send(data);
                }

                // to free some memory space
                if let JobResultData::ArchiveHash {
                    file_hash,
                    children,
                    info,
                    size,
                    content_directory_hash,
                } = x
                {
                    x = JobResultData::CachedArchiveHash {
                        content_directory_hash,
                        size,
                        file_hash,
                        info,
                        children: children
                            .into_iter()
                            .map(|x| x.get_inner_hash().clone())
                            .collect_vec(),
                    };
                }

                let node = self.tree.node_mut(result.node_id).expect("node must exist");
                node.content = JobTreeData::Finished(x);
                trace!("- mark finished");

                if let Some(parent) = self.tree.parent_ref(result.node_id) {
                    if !parent.content.is_finished() {
                        self.scheduled_jobs.insert(parent.id());
                    }
                    trace!("- schedule parent {}", parent.id());
                }

                // to free some memory space
                self.tree.remove_children(result.node_id);
            }
            JobResultData::DirectoryListing { info, children } => {
                let mut blocking = false;
                let mut cached_children = false;
                for entry in children {
                    match entry.file_type {
                        FileType::Directory => {
                            self.schedule_child(
                                result.node_id,
                                BuildJobData::DiscoverDirectory(entry),
                            );
                            blocking = true;
                        }
                        FileType::File => {
                            let cached_entry =
                                self.cache.entry(FilePath::from_realpath(&entry.path));
                            if let hash_map::Entry::Occupied(cached_entry) = cached_entry {
                                let cached = cached_entry.get();

                                if cached.modified == entry.modified
                                    && cached.size == entry.file_size
                                    && cached.file_type == HashTreeFileEntryType::File
                                {
                                    trace!("- using cached hash for file {:?}", entry.path);
                                    let data = if let Some(archive_inner_hash) =
                                        &cached.archive_inner_hash
                                    {
                                        JobResultData::CachedArchiveHash {
                                            size: cached.size,
                                            children: cached.archive_children.clone(),
                                            file_hash: cached.hash.clone(),
                                            content_directory_hash: archive_inner_hash.clone(),
                                            info: entry,
                                        }
                                    } else {
                                        JobResultData::FileHash {
                                            size: cached.size,
                                            hash: cached.hash.clone(),
                                            info: entry,
                                        }
                                    };

                                    cached_children = true;
                                    drop(cached_entry.remove());

                                    let child = JobTreeData::Finished(data);
                                    self.tree.add_child(result.node_id, child);
                                    continue;
                                }
                            }

                            self.schedule_child(result.node_id, BuildJobData::HashFile(entry));
                            blocking = true;
                        }
                        FileType::Symlink => {
                            self.schedule_child(result.node_id, BuildJobData::HashSymlink(entry));
                            blocking = true;
                        }
                        FileType::Other => {
                            let child = JobTreeData::Finished(JobResultData::Other {
                                info: entry,
                                hash: GeneralHash::NULL,
                            });
                            self.tree.add_child(result.node_id, child);
                        }
                    }
                }

                let node = self.tree.node_mut(result.node_id).expect("node must exist");
                node.content = JobTreeData::BlockedByChildren(BuildJobData::DirectoryStub(info));

                trace!("- set blocked");

                if !blocking {
                    self.scheduled_jobs.insert(result.node_id);
                    trace!("- schedule node");
                }

                if cached_children {
                    self.cache.shrink_to_fit();
                }
            }
            JobResultData::InitialEvaluation { info } => {
                let node = self.tree.node_mut(result.node_id).expect("node must exist");
                match info.file_type {
                    FileType::Directory => {
                        node.content =
                            JobTreeData::NotScheduled(BuildJobData::DiscoverDirectory(info));
                        self.scheduled_jobs.insert(result.node_id);
                    }
                    FileType::File => {
                        let cached_entry = self.cache.entry(FilePath::from_realpath(&info.path));
                        if let hash_map::Entry::Occupied(cached_entry) = cached_entry {
                            let cached = cached_entry.get();

                            if cached.modified == info.modified
                                && cached.size == info.file_size
                                && cached.file_type == HashTreeFileEntryType::File
                            {
                                trace!("- using cached hash for file {:?}", info.path);
                                let result =
                                    if let Some(archive_inner_hash) = &cached.archive_inner_hash {
                                        JobResultData::CachedArchiveHash {
                                            size: cached.size,
                                            children: cached.archive_children.clone(),
                                            file_hash: cached.hash.clone(),
                                            content_directory_hash: archive_inner_hash.clone(),
                                            info,
                                        }
                                    } else {
                                        JobResultData::FileHash {
                                            size: cached.size,
                                            hash: cached.hash.clone(),
                                            info,
                                        }
                                    };
                                node.content = JobTreeData::Finished(result);
                                self.scheduled_jobs.insert(self.tree.root_id());
                                return;
                            }
                        }

                        node.content = JobTreeData::NotScheduled(BuildJobData::HashFile(info));
                        self.scheduled_jobs.insert(result.node_id);
                    }
                    FileType::Symlink => {
                        node.content = JobTreeData::NotScheduled(BuildJobData::HashSymlink(info));
                        self.scheduled_jobs.insert(result.node_id);
                    }
                    FileType::Other => {
                        node.content = JobTreeData::Finished(JobResultData::Other {
                            info,
                            hash: GeneralHash::NULL,
                        });
                        self.scheduled_jobs.insert(self.tree.root_id());
                    }
                }
            }
        }
    }

    fn schedule_jobs(&mut self) -> ScheduleJobsResult {
        let jobs = std::mem::take(&mut self.scheduled_jobs);

        for node_id in jobs {
            //self.debug_print_graph();

            let node = self.tree.node(node_id);
            if let Some(node) = node {
                trace!("[SCHEDULE] {:?}", node);
                match &node.content {
                    JobTreeData::BlockedByChildren { .. } => match self.tree.children_ref(node) {
                        None => {
                            warn!("Encountered BLOCKED-BY-CHILDREN while no children were found: {:?}", node.content.path())
                        }
                        Some(children) => {
                            if children.iter().any(|child| {
                                matches!(
                                    child.content,
                                    JobTreeData::Finished(JobResultData::Error { .. })
                                )
                            }) || children
                                .iter()
                                .all(|child| matches!(child.content, JobTreeData::Finished { .. }))
                            {
                                trace!("- all children evaluated or error");
                                let children_nodes = children.iter().map(|x| x.content.result());

                                let mut hashes = Vec::new();
                                let mut error_found = None;
                                for child in children_nodes.flatten() {
                                    match child {
                                        JobResultData::ArchiveHash { file_hash, .. }
                                        | JobResultData::CachedArchiveHash { file_hash, .. }
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
                                        JobResultData::Error {
                                            occurred_at,
                                            reason,
                                            ..
                                        } => {
                                            error_found =
                                                Some((occurred_at.clone(), reason.clone()));
                                            break;
                                        }
                                        JobResultData::DirectoryListing { info, children: _ } => {
                                            error_found = Some((info.path.clone(), "Parent was evaluated and child did not hash properly".to_string()));
                                            break;
                                        }
                                        JobResultData::InitialEvaluation { info } => {
                                            error_found = Some((info.path.clone(), "Parent was evaluated and child was of initial eval type".to_string()));
                                            break;
                                        }
                                    }
                                }

                                if let Some((occurred_at, reason)) = error_found {
                                    trace!("- bubble up error");
                                    let mut current = Some(node_id);
                                    while let Some(node_id) = current {
                                        trace!("- setting: {node_id} to error");
                                        let node = self.tree.node_mut(node_id).unwrap();
                                        if !matches!(node.content, JobTreeData::Root) {
                                            node.content =
                                                JobTreeData::Finished(JobResultData::Error {
                                                    path: node
                                                        .content
                                                        .path()
                                                        .expect("already checked for !root")
                                                        .clone(),
                                                    occurred_at: occurred_at.clone(),
                                                    reason: reason.clone(),
                                                });
                                        };
                                        current = self.tree.parent(node_id);
                                    }
                                    self.scheduled_jobs.insert(self.tree.root_id());
                                    continue;
                                }

                                if let Some(info) = node.content.entry() {
                                    // not some for Root
                                    trace!("- prepare parent dir hash");
                                    let directory_job = BuildJobData::HashDirectory {
                                        info: info.clone(),
                                        children: hashes.into_iter().cloned().collect(),
                                    };
                                    let node =
                                        self.tree.node_mut(node_id).expect("parent must exist");
                                    node.content = JobTreeData::Running(directory_job.clone());

                                    trace!("- publish job");
                                    let job = BuildJob::new(directory_job, node.id());
                                    let job_id = job.job_id;
                                    if self.pool.publish_if_alive(job) {
                                        self.running_jobs.insert(job_id, node.id());
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
                        let immediate_children = self
                            .tree
                            .children_ref(self.tree.root_id())
                            .unwrap_or_default();
                        let mut finished = true;
                        for child in immediate_children {
                            match &child.content {
                                JobTreeData::Finished(JobResultData::Error {
                                    occurred_at,
                                    reason,
                                    ..
                                }) => {
                                    trace!("- error");
                                    return ScheduleJobsResult::Error {
                                        occurred_at: occurred_at.clone(),
                                        reason: reason.clone(),
                                    };
                                }
                                JobTreeData::Finished(_) => {}
                                _ => {
                                    finished = false;
                                    break;
                                }
                            }
                        }
                        if finished {
                            return ScheduleJobsResult::Finished;
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
                ScheduleJobsResult::Ok => {}
                ScheduleJobsResult::Error {
                    occurred_at,
                    reason,
                    ..
                } => {
                    error!(
                        "Stopped job dispatcher because of error with path {:?}: {}",
                        occurred_at, reason
                    );
                    stopping_with = Some(Err((occurred_at, reason)));
                }
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

            if self.running_jobs.is_empty() {
                if self.scheduled_jobs.is_empty() {
                    warn!("No scheduled jobs to run");
                    return Err((PathBuf::new(), "Premature exit!".to_string()));
                } else {
                    continue;
                }
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
    pub fn debug_print_graph(&mut self) {
        let current = self._debug_dotfiles.fetch_add(1, Ordering::Relaxed);

        if current > 50 {
            return;
        }

        let mut file =
            match std::fs::File::create(PathBuf::from("/tmp").join(format!("graph_{current}.dot")))
            {
                Ok(file) => file,
                Err(err) => {
                    error!("Failed to open file for debug writing {err}");
                    return;
                }
            };
        if let Err(err) = self.tree.to_dotfile(&mut file) {
            error!("Failed to write debug graph {err}");
        }
    }
}
