use crate::stages::build::cmd::worker::{GeneralHashType};
use crate::hash::GeneralHash;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use anyhow::anyhow;
use log::{error, trace};
use crate::archive::ArchiveType;
use crate::compression::CompressionType;
use crate::copy_stream::BufferCopyStreamReader;
use crate::stages::build::intermediary_build_data::{BuildArchiveFileInformation, BuildFile, BuildFileInformation};
use crate::stages::build::cmd::job::{BuildJob, JobResult};
use crate::stages::build::cmd::worker::{worker_create_error, worker_fetch_savedata, worker_publish_result_or_trigger_parent, WorkerArgument};
use crate::stages::build::cmd::worker::archive::worker_run_archive;
use crate::stages::build::output::HashTreeFileEntryType;

/// Analyze a file.
/// 
/// # Arguments
/// * `path` - The path to the file.
/// * `modified` - The last modified time of the file.
/// * `size` - The size of the file (given by fs::metadata).
/// * `id` - The id of the worker.
/// * `job` - The job to process.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
pub fn worker_run_file(path: PathBuf, modified: u64, size: u64, id: usize, job: BuildJob, result_publish: &Sender<JobResult>, job_publish: &Sender<BuildJob>, arg: &mut WorkerArgument) {
    trace!("[{}] analyzing file {} > {:?}", id, &job.target_path, path);

    match worker_fetch_savedata(arg, &job.target_path) {
        Some(found) => {
            if found.file_type == HashTreeFileEntryType::File && found.modified == modified && found.size == size {
                trace!("File {:?} is already in save file", path);
                worker_publish_result_or_trigger_parent(id, true, BuildFile::File(BuildFileInformation {
                    path: job.target_path.clone(),
                    modified,
                    content_hash: found.hash.clone(),
                    content_size: size,
                }), job, result_publish, job_publish, arg);
                return;
            }
        }
        None => {}
    }

    let archive_contents = match File::open(&path) {
        Err(err) => {
            error!("Error while opening file {:?}: {}", path, err);
            worker_publish_result_or_trigger_parent(id, false, worker_create_error(job.target_path.clone(), modified, size), job, result_publish, job_publish, arg);
            return;
        },
        Ok(stream) => {
            let archive = match arg.archives {
                true => is_archive(stream),
                false => Ok(None),
            };
            match archive {
                Err(err) => {
                    error!("Error while probing file for archive: {}", err);
                    None
                },
                Ok(None) => {
                    None
                },
                Ok(Some((archive_type, stream))) => {
                    Some(worker_run_archive(stream, archive_type, modified, size, id, arg))
                }
            }
        }
    };
    
    match fs::File::open(&path) {
        Ok(file) => {
            let mut reader = std::io::BufReader::new(file);
            let mut hash = GeneralHash::from_type(arg.hash_type);
            let content_size;

            if arg.hash_type == GeneralHashType::NULL {
                // dont hash file
                content_size = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
            } else {
                match hash.hash_file(&mut reader) {
                    Ok(size) => {
                        content_size = size;
                    }
                    Err(err) => {
                        error!("Error while hashing file {:?}: {}", path, err);
                        worker_publish_result_or_trigger_parent(id, false, worker_create_error(job.target_path.clone(), modified, size), job, result_publish, job_publish, arg);
                        return;
                    }
                }
            }

            let file = match archive_contents {
                None => {
                    BuildFile::File(BuildFileInformation {
                        path: job.target_path.clone(),
                        modified,
                        content_hash: hash,
                        content_size,
                    })
                },
                Some(archive_result) => {
                    BuildFile::ArchiveFile(BuildArchiveFileInformation {
                        path: job.target_path.clone(),
                        modified,
                        content_hash: hash,
                        content_size,
                        children: archive_result,
                    })
                }
            };
            worker_publish_result_or_trigger_parent(id, false, file, job, result_publish, job_publish, arg);
            return;
        }
        Err(err) => {
            error!("Error while opening file {:?}: {}", path, err);
            worker_publish_result_or_trigger_parent(id, false, worker_create_error(job.target_path.clone(), modified, size), job, result_publish, job_publish, arg);
            return;
        }
    }
}

/// Check if the file is an archive (potentially compressed).
///
/// # Arguments
/// * `path` - The path to the file.
///
/// # Returns
/// The archive type and the stream to the archive.
///
/// # Error
/// If the file could not be opened.
fn is_archive<R: Read + 'static>(stream: R) -> anyhow::Result<Option<(ArchiveType, Box<dyn Read>)>> {
    let stream = BufferCopyStreamReader::with_capacity(stream, CompressionType::max_stream_peek_count());
    let compression_type = CompressionType::from_stream(stream.child()).map_err(|e| anyhow!("Unable to open compressed stream: {}", e))?;

    let stream = stream.try_into_inner().map_err(|e| anyhow!("Unable to open compressed stream: {}", e))?;
    let stream = compression_type.open(stream);

    let stream = BufferCopyStreamReader::with_capacity(stream, ArchiveType::max_stream_peek_count());
    let archive_type = ArchiveType::from_stream(stream.child())
        .map_err(|e| anyhow!("Unable to determine archive stream type: {}", e))?;

    match archive_type {
        Some(archive_type) => {
            let stream = stream.try_into_inner().map_err(|e| anyhow!("Unable to open archive stream: {}", e))?;

            Ok(Some((archive_type, Box::new(stream))))
        },
        None => {
            Ok(None)
        }
    }
}
