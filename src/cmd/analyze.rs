use std::collections::HashMap;
use std::fs;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use anyhow::{anyhow, Result};
use log::{info, trace};
use crate::analyze::worker::{AnalysisJob, AnalysisResult, MarkedIntermediaryFile, WorkerArgument};
use crate::data::{Job, SaveFile, SaveFileEntry};
use crate::threadpool::ThreadPool;

pub struct AnalysisSettings {
    pub input: PathBuf,
    pub output: PathBuf,
    pub threads: Option<usize>,
}

pub fn run(analysis_settings: AnalysisSettings) -> Result<()> {
    let mut input_file_options = fs::File::options();
    input_file_options.read(true);
    input_file_options.write(false);

    let mut output_file_options = fs::File::options();
    output_file_options.create(true);
    output_file_options.write(true);
    output_file_options.truncate(true);

    let input_file = match input_file_options.open(analysis_settings.input) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open input file: {}", err));
        }
    };

    let output_file = match output_file_options.open(analysis_settings.output) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open output file: {}", err));
        }
    };

    let mut input_buf_reader = std::io::BufReader::new(&input_file);
    let mut output_buf_writer = std::io::BufWriter::new(&output_file);

    let mut save_file = SaveFile::new(&mut output_buf_writer, &mut input_buf_reader, true, true, true);
    save_file.load_header()?;

    save_file.load_all_entries_no_filter()?;
    
    let mut file_by_path = save_file.file_by_path;
    let mut file_by_path_marked = HashMap::with_capacity(file_by_path.len());
    let mut file_by_hash = save_file.file_by_hash;
    let mut all_files = save_file.all_entries;
    
    for (path, entry) in file_by_path.iter_mut() {
        file_by_path_marked.insert(path.clone(), MarkedIntermediaryFile {
            saved_file_entry: Arc::clone(entry),
            file: Arc::new(Mutex::new(None)),
        });
    }
    drop(file_by_path);
    
    // delete all entries with no collision
    
    file_by_hash.retain(|_, entry| {
        entry.len() >= 2
    });
    file_by_hash.shrink_to_fit();
    
    // delete all entries with no collision
    
    all_files.retain(|entry| {
        Arc::strong_count(entry) >= 3 // All_entries*1 + file_by_path_marked*1 + file_by_hash*1
    });
    
    let file_by_path = Arc::new(file_by_path_marked);

    // create thread pool

    let mut args = Vec::with_capacity(analysis_settings.threads.unwrap_or_else(|| num_cpus::get()));
    for _ in 0..args.capacity() {
        args.push(WorkerArgument {
            file_by_path: Arc::clone(&file_by_path)
        });
    }

    let pool: ThreadPool<AnalysisJob, AnalysisResult> = ThreadPool::new(args, crate::cmd::analyze::worker::worker_run);
    
    for entry in file_by_hash.values() {
        for entry in entry.iter() {
            pool.publish(AnalysisJob::new(Arc::clone(entry)));
        }
    }
    
    loop {
        match pool.receive_timeout(Duration::from_secs(10)) {
            Ok(result) => {
                info!("Result: {:?}", result);
            }
            Err(_) => {
                break;
            }
        }
    }

    Ok(())
}

mod worker;
pub mod analysis;