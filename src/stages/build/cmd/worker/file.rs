use crate::archive::ArchiveType;
use crate::compression::CompressionType;
use crate::copy_stream::BufferCopyStreamReader;
use crate::hash::{GeneralHash, HashingStream};
use crate::path::FilePath;
use crate::stages::build::cmd::job::{BuildJob, JobResult};
use crate::stages::build::cmd::worker::archive::{worker_run_archive, WorkerRunArchiveArguments};
use crate::stages::build::cmd::worker::GeneralHashType;
use crate::stages::build::cmd::worker::{
    worker_create_error, worker_fetch_savedata, worker_publish_result_or_trigger_parent,
    WorkerArgument,
};
use crate::stages::build::intermediary_build_data::{
    BuildArchiveFileInformation, BuildFile, BuildFileInformation,
};
use crate::stages::build::output::HashTreeFileEntryType;
use anyhow::anyhow;
use log::{error, trace};
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Arguments of the [worker_run_file] function.
///
/// # Fields
/// * `path` - The path to the file.
/// * `modified` - The last modified time of the file.
/// * `size` - The size of the file (given by fs::metadata).
/// * `id` - The id of the worker.
/// * `job` - The job to process.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
pub struct WorkerRunFileArguments<'a, 'b, 'c> {
    /// The path to the file.
    pub path: PathBuf,
    /// The last modified time of the file.
    pub modified: u64,
    /// The size of the file (given by fs::metadata).
    pub size: u64,
    /// The id of the worker.
    pub id: usize,
    /// The job to process.
    pub job: BuildJob,
    /// The channel to publish the result to.
    pub result_publish: &'a Sender<JobResult>,
    /// The channel to publish new jobs to.
    pub job_publish: &'b Sender<BuildJob>,
    /// The argument for the worker thread.
    pub arg: &'c mut WorkerArgument,
}

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
pub fn worker_run_file(arguments: WorkerRunFileArguments) {
    let WorkerRunFileArguments {
        path,
        modified,
        size,
        id,
        job,
        result_publish,
        job_publish,
        arg,
    } = arguments;

    trace!("[{}] analyzing file {} > {:?}", id, &job.target_path, path);

    if let Some(found) = worker_fetch_savedata(arg, &job.target_path) {
        if found.file_type == HashTreeFileEntryType::File
            && found.modified == modified
            && found.size == size
        {
            trace!("File {:?} is already in save file", path);
            worker_publish_result_or_trigger_parent(
                id,
                true,
                BuildFile::File(BuildFileInformation {
                    path: job.target_path.clone(),
                    modified,
                    content_hash: found.hash.clone(),
                    content_size: size,
                }),
                job,
                result_publish,
                job_publish,
                arg,
            );
            return;
        }
    }

    let stream = match File::open(&path) {
        Err(err) => {
            error!("Error while opening file {:?}: {}", path, err);
            worker_publish_result_or_trigger_parent(
                id,
                false,
                worker_create_error(job.target_path.clone(), modified, size),
                job,
                result_publish,
                job_publish,
                arg,
            );
            return;
        }
        Ok(stream) => stream,
    };
    let stream = BufReader::new(stream);

    let mut hasher = HashingStream::new(stream, arg.hash_type);

    let (archive, mut stream) = match arg.archives {
        true => {
            let stream = BufferCopyStreamReader::with_capacity_compression_peak(&mut hasher);
            let result = is_archive(stream.child());

            let stream = match stream.try_into_inner() {
                Ok(stream) => stream,
                Err(err) => {
                    error!("[{}] Error while peeking into file {:?}: {}", id, path, err);
                    worker_publish_result_or_trigger_parent(
                        id,
                        false,
                        worker_create_error(job.target_path.clone(), modified, size),
                        job,
                        result_publish,
                        job_publish,
                        arg,
                    );
                    return;
                }
            };

            (result, stream)
        }
        false => (
            Ok(None),
            BufferCopyStreamReader::with_no_capacity(&mut hasher)
                .try_into_inner()
                .unwrap(),
        ),
    };

    let archive_contents = {
        match archive {
            Err(err) => Err(anyhow!("Error while probing file for archive: {}", err)),
            Ok(None) => Ok(None),
            Ok(Some((compression_type, archive_type))) => {
                let stream = compression_type.open(&mut stream);
                worker_run_archive(
                    stream,
                    &FilePath::from_realpath(&path),
                    archive_type,
                    &mut WorkerRunArchiveArguments { id, arg },
                )
                .map(Some)
            }
        }
    };

    let archive_contents = match archive_contents {
        Err(err) => {
            error!("[{}] Error while probing file for archive: {}", id, err);
            worker_publish_result_or_trigger_parent(
                id,
                false,
                worker_create_error(job.target_path.clone(), modified, size),
                job,
                result_publish,
                job_publish,
                arg,
            );
            return;
        }
        Ok(content) => content,
    };

    // finalize hashing
    let content_size = if arg.hash_type != GeneralHashType::NULL {
        match std::io::copy(&mut stream, &mut std::io::sink()) {
            Ok(_) => {}
            Err(err) => {
                error!("Error while hashing file {:?}: {}", path, err);
                worker_publish_result_or_trigger_parent(
                    id,
                    false,
                    worker_create_error(job.target_path.clone(), modified, size),
                    job,
                    result_publish,
                    job_publish,
                    arg,
                );
                return;
            }
        }
        drop(stream);
        hasher.bytes_processed()
    } else {
        fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    };

    let hash = hasher.hash();

    let file = match archive_contents {
        None => BuildFile::File(BuildFileInformation {
            path: job.target_path.clone(),
            modified,
            content_hash: hash,
            content_size,
        }),
        Some(mut archive_result) => {
            let directory_hash = {
                archive_result.sort_by(|a, b| {
                    a.get_content_hash()
                        .partial_cmp(b.get_content_hash())
                        .expect("Two hashes must compare to each other")
                });
                let mut hash = GeneralHash::from_type(arg.hash_type);
                let _ = hash.hash_directory(archive_result.iter());
                hash
            };
            BuildFile::ArchiveFile(BuildArchiveFileInformation {
                path: job.target_path.clone(),
                modified,
                file_hash: hash,
                directory_hash,
                content_size,
                children: archive_result,
            })
        }
    };
    worker_publish_result_or_trigger_parent(id, false, file, job, result_publish, job_publish, arg);
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
/// If the stream could not be read
pub fn is_archive<R: Read>(
    stream: BufferCopyStreamReader<R>,
) -> anyhow::Result<Option<(CompressionType, ArchiveType)>> {
    let compression_type = CompressionType::from_stream(stream.child())
        .map_err(|e| anyhow!("Unable to open compressed stream: {}", e))?;

    let stream = compression_type.open(stream);

    let stream = BufferCopyStreamReader::with_capacity_archive_peak(stream);
    let archive_type = ArchiveType::from_stream(stream.child())
        .map_err(|e| anyhow!("Unable to determine archive stream type: {}", e))?;

    match archive_type {
        Some(archive_type) => Ok(Some((compression_type, archive_type))),
        None => Ok(None),
    }
}
