use std::collections::HashMap;
use std::fs;
use std::path::{PathBuf};
use std::sync::Arc;
use anyhow::{anyhow, Result};
use serde::Serialize;
use crate::build::worker::{worker_run, WorkerArgument};
use crate::data::{FilePath, GeneralHashType, Job, PathTarget, ResultTrait, File, SaveFile, SaveFileEntryRef, SaveFileEntry};
use crate::threadpool::ThreadPool;

mod worker;

pub struct BuildSettings {
    pub directory: PathBuf,
    // pub into_archives: bool,
    pub follow_symlinks: bool,
    pub output: PathBuf,
    pub absolute_paths: bool,
    pub threads: Option<usize>,
    
    pub hash_type: GeneralHashType,
    pub continue_file: bool,
}

#[derive(Debug, Serialize, Clone)]
struct JobResultContent {
    already_cached: bool,
    content: File,
}

#[derive(Debug, Serialize, Clone)]
enum JobResult {
    Final(JobResultContent),
    Intermediate(JobResultContent),
}

impl ResultTrait for JobResult {
    
}

pub fn run(
    build_settings: BuildSettings,
) -> Result<()> {
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
    
    let mut save_file = SaveFile::new(&mut result_out, &mut result_in, false, true, false);
    match save_file.load_header() {
        Ok(_) => {},
        Err(err) => {
            if build_settings.continue_file && existed {
                return Err(anyhow!("Failed to load header from result file: {}. Delete the output file or provide the --override flag to override", err));
            } else {
                save_file.save_header()?;
            }
        }
    }
    
    match save_file.load_all_entries_no_filter() {
        Ok(_) => {},
        Err(err) => {
            return Err(anyhow!("Failed to load entries from result file: {}. Delete the output file or provide the --override flag to override", err));
        }
    }

    // dont need hash -> file mapping
    save_file.empty_file_by_hash();
    save_file.empty_entry_list();
    
    let mut file_by_hash: HashMap<FilePath, SaveFileEntry> = HashMap::with_capacity(save_file.file_by_hash.len());
    save_file.file_by_path.drain().for_each(|(k, v)| {
        file_by_hash.insert(k, Arc::into_inner(v).expect("There should be no further references to the entry"));
    });
    let file_by_hash = Arc::new(file_by_hash);

    // create thread pool

    let mut args = Vec::with_capacity(build_settings.threads.unwrap_or_else(|| num_cpus::get()));
    for _ in 0..args.capacity() {
        args.push(WorkerArgument {
            follow_symlinks: build_settings.follow_symlinks,
            hash_type: build_settings.hash_type,
            save_file_by_path: Arc::clone(&file_by_hash),
        });
    }
    
    let pool: ThreadPool<Job, JobResult> = ThreadPool::new(args, worker_run);

    let root_file = FilePath::from_path(build_settings.directory, PathTarget::File);
    let root_job = Job::new(None, root_file);
    
    pool.publish(root_job);

    while let Ok(result) = pool.receive() {
        let finished;
        let result = match result {
            JobResult::Intermediate(inner) => {
                finished = false;
                inner
            },
            JobResult::Final(inner) => {
                finished = true;
                inner
            },
        };
        
        if !result.already_cached {
            let entry = SaveFileEntryRef::from(&result.content);
            save_file.write_entry_ref(&entry)?;
        }
        
        if finished {
            break;
        }
    }
    
    return Ok(());
}
