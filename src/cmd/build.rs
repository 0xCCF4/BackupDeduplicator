use std::fs;
use std::io::{BufWriter, Write};
use std::path::{PathBuf};
use anyhow::{anyhow, Result};
use serde::Serialize;
use crate::build::worker::{worker_run, WorkerArgument};
use crate::data::{FilePath, GeneralHashType, Job, PathTarget, ResultTrait, File, SaveFile};
use crate::threadpool::ThreadPool;

mod worker;

pub struct BuildSettings {
    pub directory: PathBuf,
    pub into_archives: bool,
    pub follow_symlinks: bool,
    pub output: PathBuf,
    pub absolute_paths: bool,
    pub threads: Option<usize>,
    
    pub hash_type: GeneralHashType,
    pub continue_file: bool,
}

#[derive(Debug, Serialize, Clone)]
enum JobResult {
    Final(File),
    Intermediate(File),
}

impl ResultTrait for JobResult {
    
}

pub fn run(
    build_settings: BuildSettings,
) -> Result<()> {
    let mut args = Vec::with_capacity(build_settings.threads.unwrap_or_else(|| num_cpus::get()));
    for _ in 0..args.capacity() {
        args.push(WorkerArgument {
            follow_symlinks: build_settings.follow_symlinks,
            hash_type: build_settings.hash_type,
        });
    }
    
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
    
    let mut save_file = SaveFile::new();
    match save_file.load_header(&mut result_in) {
        Ok(_) => {},
        Err(err) => {
            if build_settings.continue_file && existed {
                return Err(anyhow!("Failed to load header from result file: {}. Delete the output file or provide the --override flag to override", err));
            } else {
                save_file.save_header(&mut result_out)?;
            }
        }
    }
    
    let pool: ThreadPool<Job, JobResult> = ThreadPool::new(args, worker_run);

    let root_file = FilePath::from_path(build_settings.directory, PathTarget::File);
    let root_job = Job::new(None, root_file);
    
    pool.publish(root_job);

    while let Ok(result) = pool.receive() {
        write_file_record(&mut result_out, &result)?;
        
        if let JobResult::Final(_) = result {
            break;
        }
    }
    
    return Ok(());
}

fn write_file_record(writer: &mut BufWriter<&fs::File>, result: &JobResult) -> Result<()> {
    let result = match result {
        JobResult::Final(file) => file,
        JobResult::Intermediate(file) => file,
    };
    let string = serde_json::to_string(result)?;
    writer.write(string.as_bytes())?;
    writer.write("\n".as_bytes())?;
    writer.flush()?;
    Ok(())
}
