use crate::hash::{GeneralHash, GeneralHashType};
use crate::pool::ThreadPool;
use crate::stages::analyze::intermediary_analysis_data::AnalysisFile;
use crate::stages::analyze::output::DupSetEntryRef;
use crate::stages::analyze::worker::AnalysisIntermediaryFile;
use crate::stages::analyze::worker::{
    worker_run, AnalysisJob, AnalysisResult, AnalysisWorkerArgument,
};
use crate::stages::build::output::{HashTreeFile, HashTreeFileEntry, HashTreeFileEntryType};
use crate::utils::NullWriter;
use anyhow::{anyhow, Result};
use log::{error, info, trace};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The settings for the analysis cmd.
///
/// # Fields
/// * `input` - The input file to analyze.
/// * `output` - The output file to write the results to.
/// * `threads` - The number of threads to use for the analysis. If None, the number of threads is equal to the number of CPUs.
pub struct AnalysisSettings {
    /// The input file to analyze.
    pub input: PathBuf,
    /// The output file to write the results to.
    pub output: PathBuf,
    /// The number of threads to use for the analysis. If None, the number of threads is equal to the number of CPUs.
    pub threads: Option<usize>,
}

/// Run the analysis cmd.
///
/// # Arguments
/// * `analysis_settings` - The settings for the analysis cmd.
///
/// # Returns
/// Nothing
///
/// # Errors
/// * If the input file cannot be opened.
/// * If the output file cannot be opened.
/// * If the header of the input file cannot be loaded.
/// * If an error occurs while loading entries from the input file.
/// * If writing to the output file fails.
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
    let mut null_out_writer = NullWriter::default();
    let mut output_buf_writer = std::io::BufWriter::new(&output_file);

    let mut save_file = HashTreeFile::new(
        &mut null_out_writer,
        &mut input_buf_reader,
        GeneralHashType::NULL,
        true,
        true,
        true,
    );
    save_file.load_header()?;

    save_file.load_all_entries_no_filter()?;

    let mut file_by_path = save_file.file_by_path;
    let mut file_by_path_marked = HashMap::with_capacity(file_by_path.len());
    let mut file_by_hash = save_file.file_by_hash;
    let mut all_files = save_file.all_entries;

    for (path, entry) in file_by_path.iter_mut() {
        file_by_path_marked.insert(
            path.clone(),
            AnalysisIntermediaryFile {
                saved_file_entry: Arc::clone(entry),
                file: Arc::new(Mutex::new(None)),
            },
        );
    }
    drop(file_by_path);

    // delete all entries with no collision

    file_by_hash.retain(|_, entry| entry.len() >= 2);
    file_by_hash.shrink_to_fit();

    // delete all entries with no collision

    all_files.retain(|entry| {
        Arc::strong_count(entry) >= 3 // All_entries*1 + file_by_path_marked*1 + file_by_hash*1
    });

    let file_by_path = Arc::new(file_by_path_marked);

    // create thread pool

    let mut args = Vec::with_capacity(analysis_settings.threads.unwrap_or_else(num_cpus::get));
    for _ in 0..args.capacity() {
        args.push(AnalysisWorkerArgument {
            file_by_path: Arc::clone(&file_by_path),
        });
    }

    let pool: ThreadPool<AnalysisJob, AnalysisResult> = ThreadPool::new(args, worker_run);

    for entry in &all_files {
        pool.publish(AnalysisJob::new(Arc::clone(entry)));
    }

    while let Ok(result) = pool.receive_timeout(Duration::from_secs(10)) {
        info!("Result: {:?}", result);
    }

    drop(pool);

    let mut duplicated_bytes: u64 = 0;

    for entry in &all_files {
        trace!("File: {}", entry.path);
        let file = file_by_path.get(&entry.path).unwrap();
        let file = file.file.lock().unwrap();
        if let Some(file) = file.deref() {
            let parent = file.parent().lock().unwrap();
            match parent.deref() {
                Some(parent) => {
                    // check if parent is also conflicting

                    let parent = parent.upgrade().unwrap();
                    let parent_hash = match parent.deref() {
                        AnalysisFile::File(info) => Some(&info.content_hash),
                        AnalysisFile::Directory(info) => Some(&info.content_hash),
                        AnalysisFile::Symlink(info) => Some(&info.content_hash),
                        AnalysisFile::Other(_) => None,
                    };

                    let parent_conflicting = match parent_hash {
                        None => false,
                        Some(parent_hash) => match file_by_hash.get(parent_hash) {
                            Some(entries) => entries.len() >= 2,
                            None => false,
                        },
                    };

                    if !parent_conflicting {
                        duplicated_bytes +=
                            write_result_entry(file, &file_by_hash, &mut output_buf_writer);
                    }
                }
                None => {
                    duplicated_bytes +=
                        write_result_entry(file, &file_by_hash, &mut output_buf_writer);
                }
            }
        } else {
            error!("File not analyzed yet: {:?}", entry.path);
        }
    }

    output_buf_writer.flush().expect("Unable to flush file");

    print!(
        "There are {} GB of duplicated files",
        duplicated_bytes / 1024 / 1024 / 1024
    );

    Ok(())
}

/// Used to find duplicates of entries in the hash tree file.
#[derive(Debug, PartialEq, Hash, Eq)]
struct SetKey<'a> {
    size: u64,
    ftype: &'a HashTreeFileEntryType,
    children: &'a Vec<GeneralHash>,
}
/// Write the result entry to the output file. Find all duplicates of the file and write them to the output file.
/// If called for every file, it will write all duplicates to the output file.
/// Writing each file only once
fn write_result_entry(
    file: &AnalysisFile,
    file_by_hash: &HashMap<GeneralHash, Vec<Arc<HashTreeFileEntry>>>,
    output_buf_writer: &mut std::io::BufWriter<&fs::File>,
) -> u64 {
    let hash = match file {
        AnalysisFile::File(info) => &info.content_hash,
        AnalysisFile::Directory(info) => &info.content_hash,
        AnalysisFile::Symlink(info) => &info.content_hash,
        AnalysisFile::Other(_) => {
            return 0;
        }
    };

    let mut sets: HashMap<SetKey, Vec<&HashTreeFileEntry>> = HashMap::new();

    for file in file_by_hash.get(hash).unwrap() {
        sets.entry(SetKey {
            size: file.size,
            ftype: &file.file_type,
            children: &file.children,
        })
        .or_default()
        .push(file);
    }

    let mut result_size: u64 = 0;

    for set in &sets {
        if set.1.len() <= 1 {
            continue;
        }

        if &set.1[0].path != file.path() {
            // no duplicates
            continue;
        }

        let mut conflicting = Vec::with_capacity(set.1.len());
        for file in set.1 {
            conflicting.push(&file.path);
        }

        let result = DupSetEntryRef {
            ftype: set.0.ftype,
            size: set.0.size,
            hash,
            conflicting,
        };
        let _ = output_buf_writer
            .write(serde_json::to_string(&result).unwrap().as_bytes())
            .expect("Unable to write to file");
        let _ = output_buf_writer
            .write('\n'.to_string().as_bytes())
            .expect("Unable to write to file");

        result_size += result.size * (result.conflicting.len() as u64 - 1);
    }

    result_size
}
