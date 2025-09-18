use crate::hash::{GeneralHash, GeneralHashType};
use crate::path::FilePath;
use crate::pool::{JobTrait, ResultTrait};
use crate::shallow_ref_tree::NodeId;
use crate::stages::build::cmd::archive::ArchiveFile;
use crate::stages::build::cmd::planner::JobPlanner;
use crate::stages::build::output::{HashTreeFile, HashTreeFileEntry};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
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

    // create buf reader and writer
    let mut result_in = std::io::BufReader::new(if build_settings.continue_file {
        Box::new(&result_file) as Box<dyn Read>
    } else {
        Box::new(std::io::empty()) as Box<dyn Read>
    });
    let mut result_out = std::io::BufWriter::new(&result_file);

    let mut save_file = HashTreeFile::new(
        &mut result_out,
        &mut result_in,
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

    let mut planner = JobPlanner::new(threads, &build_settings);

    for root_file in build_settings.directory.iter() {
        planner.schedule_initial_job(root_file.clone());
    }

    planner.run();

    Ok(())
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
    DiscoverDirectory(PathBuf),
    HashFile(PathBuf),
    HashSymlink(PathBuf),
    DirectoryStub(PathBuf),
    HashDirectory {
        path: PathBuf,
        children: Vec<GeneralHash>,
    },
    Initial(PathBuf),
}

impl BuildJobData {
    pub fn path(&self) -> &PathBuf {
        match self {
            BuildJobData::DiscoverDirectory(path) => path,
            BuildJobData::DirectoryStub(path) => path,
            BuildJobData::HashFile(path) => path,
            BuildJobData::HashSymlink(path) => path,
            BuildJobData::HashDirectory { path, .. } => path,
            BuildJobData::Initial(path) => path,
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
        path: PathBuf,
        children: Vec<DirectoryEntry>,
    },
    FileHash {
        path: PathBuf,
        hash: GeneralHash,
        size: u64,
    },
    ArchiveHash {
        path: PathBuf,
        size: u64,
        file_hash: GeneralHash,
        children: Vec<ArchiveFile>,
        content_directory_hash: GeneralHash,
    },
    DirectoryHash {
        path: PathBuf,
        hash: GeneralHash,
    },
    SymlinkHash {
        path: PathBuf,
        hash: GeneralHash,
    },
    Error {
        path: PathBuf,
        occurred_at: PathBuf,
        reason: String,
    },
    Other {
        path: PathBuf,
        hash: GeneralHash,
    },
    InitialEvaluation(DirectoryEntry),
}

impl JobResultData {
    pub fn path(&self) -> &PathBuf {
        match self {
            JobResultData::DirectoryListing { path, .. } => path,
            JobResultData::FileHash { path, .. } => path,
            JobResultData::ArchiveHash { path, .. } => path,
            JobResultData::DirectoryHash { path, .. } => path,
            JobResultData::SymlinkHash { path, .. } => path,
            JobResultData::Error { path, .. } => path,
            JobResultData::Other { path, .. } => path,
            JobResultData::InitialEvaluation(entry) => &entry.path,
        }
    }
}

#[derive(Debug)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug)]
pub struct DirectoryEntry {
    pub path: PathBuf,
    pub modified: u64,
    pub file_type: FileType,
    pub file_size: u64,
}
