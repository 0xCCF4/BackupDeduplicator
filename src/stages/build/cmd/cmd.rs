use crate::hash::{GeneralHash, GeneralHashType};
use crate::path::FilePath;
use crate::pool::{JobTrait, ResultTrait};
use crate::shallow_ref_tree::NodeId;
use crate::stages::build::cmd::archive::ArchiveFile;
use crate::stages::build::cmd::planner::JobPlanner;
use crate::stages::build::output::{HashTreeFile, HashTreeFileEntry, HashTreeFileEntryType};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::Write;
use std::io::{Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::{fs, panic, process};

/// The settings for the build command.
///
/// # Fields
/// * `directory` - The directories to build hash tree for.
/// * `into_archives` - Whether to traverse into archives.
/// * `follow_symlinks` - Whether to follow symlinks when traversing the file system.
/// * `output` - The output file to write the hash tree to.
/// * `threads` - The number of threads to use for building the hash tree. None = number of logical CPUs.
/// * `hash_type` - The hash algorithm to use for hashing files.
/// * `continue_file` - Whether to continue an existing hash tree file.
pub struct BuildSettings {
    /// The directory to build hash tree for.
    pub directory: Vec<PathBuf>,
    /// Whether to traverse into archives.
    pub into_archives: bool,
    /// Whether to follow symlinks when traversing the file system.
    pub follow_symlinks: bool,
    /// The output file to write the hash tree to.
    pub output: PathBuf,
    // pub absolute_paths: bool,
    /// The number of threads to use for building the hash tree. None = number of logical CPUs.
    pub threads: Option<usize>,
    /// The hash algorithm to use for hashing files.
    pub hash_type: GeneralHashType,
    /// Whether to continue an existing hash tree file.
    pub continue_file: bool,
}

/// Runs the build command. Hashes a directory and produces a hash tree file.
///
/// # Arguments
/// * `build_settings` - The settings for the build command.
///
/// # Returns
/// Nothing
///
/// # Errors
/// * If the output file cannot be opened.
/// * If the header cannot be loaded from the output file (if the file is continued).
/// * If the output file cannot be written to.
pub fn run(build_settings: BuildSettings) -> Result<()> {
    let existed = build_settings.output.exists();
    let mut result_file_options = fs::File::options();

    result_file_options.create(true);

    if build_settings.continue_file {
        result_file_options.append(true).read(true);
    } else {
        result_file_options.write(true);
    }

    let result_file = match result_file_options.open(&build_settings.output) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open result file: {}", err));
        }
    };

    let mut reader = std::io::BufReader::new(&result_file);

    // create buf reader and writer
    let result_in = if build_settings.continue_file {
        Some(&mut reader)
    } else {
        None
    };
    let mut result_out = std::io::BufWriter::new(&result_file);

    let mut save_file = HashTreeFile::new(
        &mut result_out,
        result_in,
        build_settings.hash_type,
        false,
        true,
        false,
    );
    match save_file.load_header() {
        Ok(_) => {}
        Err(err) => {
            if build_settings.continue_file && existed {
                return Err(anyhow!("Failed to load header from result file: {}. Delete the output file or provide the --overwrite flag to override", err));
            } else {
                save_file.save_header()?;
            }
        }
    }

    // load all existing entries from the hash tree file
    match save_file.load_all_entries_no_filter() {
        Ok(_) => {}
        Err(err) => {
            return Err(anyhow!("Failed to load entries from result file: {}. Delete the output file or provide the --override flag to override", err));
        }
    }

    // dont need hash -> file mapping
    save_file.empty_file_by_hash();
    save_file.empty_entry_list();

    let mut file_by_hash: HashMap<FilePath, HashTreeFileEntry> =
        HashMap::with_capacity(save_file.file_by_hash.len());
    save_file.file_by_path.drain().for_each(|(k, v)| {
        file_by_hash.insert(
            k,
            Arc::into_inner(v).expect("There should be no further references to the entry"),
        );
    });

    let threads = build_settings.threads.unwrap_or_else(num_cpus::get);

    // panic if thread panics
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        orig_hook(panic_info);
        process::exit(1);
    }));

    let (result_sender, result_receiver) = channel();

    drop(save_file);
    drop(result_out);

    let result_file_cloned = match result_file.try_clone() {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to clone result file handle: {}", err));
        }
    };
    std::thread::spawn(move || {
        let mut result_out = std::io::BufWriter::new(result_file_cloned);

        if let Err(err) = result_out.seek(SeekFrom::End(0)) {
            eprintln!("Failed to seek to end of result file: {err}");
            process::exit(1);
        }

        writeln!(result_out, "").unwrap_or_else(|err| {
            eprintln!("Failed to write to result file: {err}");
            process::exit(1);
        });

        let save_file = HashTreeFile::new(
            &mut result_out,
            None::<&mut std::io::BufReader<std::fs::File>>,
            build_settings.hash_type,
            false,
            false,
            false,
        );

        while let Ok(data) = result_receiver.recv() {
            save_file.write_entry(&data).unwrap_or_else(|err| {
                eprintln!("Failed to write entry to result file: {err}");
                process::exit(1);
            });
        }
    });

    let mut planner = JobPlanner::new(threads, &build_settings, &file_by_hash, result_sender);

    for root_file in build_settings.directory.iter() {
        planner.schedule_initial_job(root_file.clone());
    }

    planner.run().map_err(|(path, reason)| {
        anyhow!("Building hash tree failed on {path:?} because {reason:?}")
    })
}

#[derive(Debug)]
pub struct BuildJob {
    pub job_id: u64,
    pub node_id: NodeId,
    pub data: BuildJobData,
}

impl JobTrait for BuildJob {
    fn job_id(&self) -> u64 {
        self.job_id
    }
}

static JOB_ID_COUNTER: Mutex<u64> = Mutex::new(0);

impl BuildJob {
    fn new_job_id() -> u64 {
        let mut counter = JOB_ID_COUNTER.lock().expect("Failed to lock job counter");
        *counter += 1;
        *counter
    }
    pub fn new(data: BuildJobData, owning_node: NodeId) -> Self {
        Self {
            job_id: BuildJob::new_job_id(),
            node_id: owning_node,
            data,
        }
    }
    pub fn result(&self) -> JobResultBuilder {
        JobResultBuilder {
            job_id: self.job_id,
            node_id: self.node_id,
        }
    }
}

pub struct JobResultBuilder {
    pub job_id: u64,
    pub node_id: NodeId,
}

impl JobResultBuilder {
    pub fn build(self, result: JobResultData) -> JobResult {
        JobResult {
            job_id: self.job_id,
            node_id: self.node_id,
            result,
        }
    }
}

#[derive(Debug, Clone)]
pub enum BuildJobData {
    DiscoverDirectory(DirectoryEntry),
    HashFile(DirectoryEntry),
    HashSymlink(DirectoryEntry),
    DirectoryStub(DirectoryEntry),
    HashDirectory {
        info: DirectoryEntry,
        children: Vec<GeneralHash>,
    },
    Initial(PathBuf),
}

impl BuildJobData {
    pub fn path(&self) -> &PathBuf {
        match self {
            BuildJobData::DiscoverDirectory(info) => &info.path,
            BuildJobData::DirectoryStub(info) => &info.path,
            BuildJobData::HashFile(info) => &info.path,
            BuildJobData::HashSymlink(info) => &info.path,
            BuildJobData::HashDirectory { info, .. } => &info.path,
            BuildJobData::Initial(path) => path,
        }
    }
    pub fn entry(&self) -> Option<&DirectoryEntry> {
        match self {
            BuildJobData::DiscoverDirectory(info) => Some(info),
            BuildJobData::DirectoryStub(info) => Some(info),
            BuildJobData::HashFile(info) => Some(info),
            BuildJobData::HashSymlink(info) => Some(info),
            BuildJobData::HashDirectory { info, .. } => Some(info),
            BuildJobData::Initial(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct JobResult {
    pub job_id: u64,
    pub node_id: NodeId,
    pub result: JobResultData,
}

impl ResultTrait for JobResult {}

#[derive(Debug)]
pub enum JobResultData {
    DirectoryListing {
        info: DirectoryEntry,
        children: Vec<DirectoryEntry>,
    },
    FileHash {
        info: DirectoryEntry,
        hash: GeneralHash,
        size: u64,
    },
    ArchiveHash {
        info: DirectoryEntry,
        size: u64,
        file_hash: GeneralHash,
        children: Vec<ArchiveFile>,
        content_directory_hash: GeneralHash,
    },
    CachedArchiveHash {
        info: DirectoryEntry,
        size: u64,
        file_hash: GeneralHash,
        children: Vec<GeneralHash>,
        content_directory_hash: GeneralHash,
    },
    DirectoryHash {
        info: DirectoryEntry,
        hash: GeneralHash,
        children: Vec<GeneralHash>,
    },
    SymlinkHash {
        info: DirectoryEntry,
        hash: GeneralHash,
    },
    Error {
        path: PathBuf,
        occurred_at: PathBuf,
        reason: String,
    },
    Other {
        info: DirectoryEntry,
        hash: GeneralHash,
    },
    InitialEvaluation {
        info: DirectoryEntry,
    },
}

impl JobResultData {
    pub fn path(&self) -> &PathBuf {
        match self {
            JobResultData::DirectoryListing { info, .. } => &info.path,
            JobResultData::FileHash { info, .. } => &info.path,
            JobResultData::ArchiveHash { info, .. } => &info.path,
            JobResultData::CachedArchiveHash { info, .. } => &info.path,
            JobResultData::DirectoryHash { info, .. } => &info.path,
            JobResultData::SymlinkHash { info, .. } => &info.path,
            JobResultData::Error { path, .. } => path,
            JobResultData::Other { info, .. } => &info.path,
            JobResultData::InitialEvaluation { info } => &info.path,
        }
    }
    pub fn entry(&self) -> Option<&DirectoryEntry> {
        match self {
            JobResultData::DirectoryListing { info, .. } => Some(info),
            JobResultData::FileHash { info, .. } => Some(info),
            JobResultData::ArchiveHash { info, .. } => Some(info),
            JobResultData::CachedArchiveHash { info, .. } => Some(info),
            JobResultData::DirectoryHash { info, .. } => Some(info),
            JobResultData::SymlinkHash { info, .. } => Some(info),
            JobResultData::Error { .. } => None,
            JobResultData::Other { info, .. } => Some(info),
            JobResultData::InitialEvaluation { info, .. } => Some(info),
        }
    }
    pub fn hash_tree_file_entry(&self) -> Vec<HashTreeFileEntry> {
        match self {
            JobResultData::DirectoryHash {
                info,
                hash,
                children,
            } => {
                vec![HashTreeFileEntry {
                    children: children.clone(),
                    file_type: HashTreeFileEntryType::Directory,
                    modified: info.modified,
                    size: children.len() as u64,
                    path: FilePath::from_realpath(&info.path),
                    hash: hash.clone(),
                    archive_inner_hash: None,
                    archive_children: Vec::new(),
                }]
            }
            JobResultData::ArchiveHash {
                info,
                file_hash,
                children,
                size,
                content_directory_hash,
            } => {
                let mut result = Vec::new();
                result.push(HashTreeFileEntry {
                    archive_children: children
                        .into_iter()
                        .map(|child| child.get_file_hash().clone())
                        .collect(),
                    file_type: HashTreeFileEntryType::File,
                    size: *size,
                    modified: info.modified,
                    children: vec![],
                    archive_inner_hash: Some(content_directory_hash.clone()),
                    hash: file_hash.clone(),
                    path: FilePath::from_realpath(&info.path),
                });

                for child in children {
                    result.extend(child.to_hash_file_entry());
                }

                result
            }
            JobResultData::CachedArchiveHash {
                info,
                file_hash,
                children,
                size,
                content_directory_hash,
            } => {
                let mut result = Vec::new();
                result.push(HashTreeFileEntry {
                    archive_children: children.clone(),
                    file_type: HashTreeFileEntryType::File,
                    size: *size,
                    modified: info.modified,
                    children: vec![],
                    archive_inner_hash: Some(content_directory_hash.clone()),
                    hash: file_hash.clone(),
                    path: FilePath::from_realpath(&info.path),
                });

                result
            }
            JobResultData::FileHash { info, hash, size } => vec![HashTreeFileEntry {
                children: Vec::new(),
                file_type: HashTreeFileEntryType::File,
                modified: info.modified,
                size: *size,
                path: FilePath::from_realpath(&info.path),
                hash: hash.clone(),
                archive_inner_hash: None,
                archive_children: Vec::new(),
            }],
            JobResultData::SymlinkHash { info, hash } => vec![HashTreeFileEntry {
                children: Vec::new(),
                file_type: HashTreeFileEntryType::Symlink,
                modified: info.modified,
                size: info.file_size,
                path: FilePath::from_realpath(&info.path),
                hash: hash.clone(),
                archive_inner_hash: None,
                archive_children: Vec::new(),
            }],
            JobResultData::Other { info, hash } => vec![HashTreeFileEntry {
                children: Vec::new(),
                file_type: HashTreeFileEntryType::Other,
                modified: info.modified,
                size: info.file_size,
                archive_inner_hash: None,
                archive_children: Vec::new(),
                path: FilePath::from_realpath(&info.path),
                hash: hash.clone(),
            }],
            JobResultData::InitialEvaluation { .. } => vec![],
            JobResultData::DirectoryListing { .. } => vec![],
            JobResultData::Error { .. } => vec![],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub path: PathBuf,
    pub modified: u64,
    pub file_type: FileType,
    pub file_size: u64,
}
