use crate::hash::GeneralHashType;
use crate::path::FilePath;
use crate::pool::ThreadPool;
use crate::stages::build::cmd::job::{BuildJob, JobResult};
use crate::stages::build::cmd::worker::{worker_run, WorkerArgument};
use crate::stages::build::output::{HashTreeFile, HashTreeFileEntry, HashTreeFileEntryRef};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

/// The settings for the build command.
///
/// # Fields
/// * `directory` - The directory to build.
/// * `into_archives` - Whether to traverse into archives.
/// * `follow_symlinks` - Whether to follow symlinks when traversing the file system.
/// * `output` - The output file to write the hash tree to.
/// * `threads` - The number of threads to use for building the hash tree. None = number of logical CPUs.
/// * `hash_type` - The hash algorithm to use for hashing files.
/// * `continue_file` - Whether to continue an existing hash tree file.
pub struct BuildSettings {
    /// The directory to build.
    pub directory: PathBuf,
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

    let result_file = match result_file_options.open(build_settings.output) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open result file: {}", err));
        }
    };

    // create buf reader and writer
    let mut result_in = std::io::BufReader::new(&result_file);
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
                return Err(anyhow!("Failed to load header from result file: {}. Delete the output file or provide the --override flag to override", err));
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
    let file_by_hash = Arc::new(file_by_hash);

    // create thread pool

    let mut args = Vec::with_capacity(build_settings.threads.unwrap_or_else(num_cpus::get));
    for _ in 0..args.capacity() {
        args.push(WorkerArgument {
            archives: build_settings.into_archives,
            follow_symlinks: build_settings.follow_symlinks,
            hash_type: build_settings.hash_type,
            save_file_by_path: Arc::clone(&file_by_hash),
        });
    }

    let pool: ThreadPool<BuildJob, JobResult> = ThreadPool::new(args, worker_run);

    let root_file = FilePath::from_realpath(build_settings.directory);
    let root_job = BuildJob::new(None, root_file);

    pool.publish(root_job);

    while let Ok(result) = pool.receive() {
        let finished;
        let result = match result {
            JobResult::Intermediate(inner) => {
                finished = false;
                inner
            }
            JobResult::Final(inner) => {
                finished = true;
                inner
            }
        };

        if !result.already_cached {
            let entry = HashTreeFileEntryRef::from(&result.content);
            save_file.write_entry_ref(&entry)?;
        }

        if finished {
            break;
        }
    }

    Ok(())
}
