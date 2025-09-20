use crate::archive::ArchiveType;
use crate::compression::CompressionType;
use crate::copy_stream::BufferCopyStreamReader;
use crate::hash::{GeneralHash, GeneralHashType, HashingStream};
use crate::path::FilePath;
use crate::stages::build::cmd::archive::{worker_run_archive, WorkerRunArchiveArguments};
use crate::stages::build::cmd::{
    BuildJob, BuildJobData, DirectoryEntry, FileType, JobResult, JobResultData,
};
use anyhow::anyhow;
use log::{debug, error};
use std::fs;
use std::fs::{File, Metadata};
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::SystemTime;

/// The argument for the worker main thread.
///
/// # Fields
/// * `follow_symlinks` - Whether to follow symlinks when traversing the file system.
/// * `archives` - Whether to traverse into archives.
/// * `hash_type` - The hash algorithm to use for hashing files.
/// * `save_file_by_path` - A hash map of [FilePath] -> [HashTreeFileEntry].
pub struct WorkerArgument {
    /// Whether to follow symlinks when traversing the file system.
    pub follow_symlinks: bool,
    /// Whether to traverse into archives.
    pub archives: bool,
    /// The hash algorithm to use for hashing files.
    pub hash_type: GeneralHashType,
}

fn evaluate_file(id: usize, path: PathBuf, metadata: &Metadata) -> DirectoryEntry {
    let modified = metadata
        .modified()
        .map(|m| {
            m.duration_since(SystemTime::UNIX_EPOCH)
                .map(|e| e.as_secs())
                .unwrap_or_else(|e| {
                    error!(
                    "[{id}] Error calculating elapsed time for {path:?}: {e}. Using default value."
                );
                    0
                })
        })
        .unwrap_or_else(|e| {
            error!(
                "[{id}] Error getting modification time for {path:?}: {e}. Using default value."
            );
            0
        });

    let file_type = if metadata.is_dir() {
        FileType::Directory
    } else if metadata.is_file() {
        FileType::File
    } else if metadata.is_symlink() {
        FileType::Symlink
    } else {
        FileType::Other
    };

    DirectoryEntry {
        path,
        modified,
        file_size: metadata.len(),
        file_type,
    }
}

/// Main function for the worker thread.
///
/// # Arguments
/// * `id` - The id of the worker.
/// * `job` - The job to process.
/// * `result_publish` - The channel to publish the result to.
/// * `job_publish` - The channel to publish new jobs to.
/// * `arg` - The argument for the worker thread.
pub fn worker_run(
    id: usize,
    job: BuildJob,
    result_publish: &Sender<JobResult>,
    arg: &mut WorkerArgument,
) {
    let result_builder = job.result();

    match job.data {
        BuildJobData::DiscoverDirectory(info) => {
            let dir = match fs::read_dir(&info.path) {
                Ok(dir) => dir,
                Err(e) => {
                    error!("[{id}] Error reading directory {:?}: {e}.", info.path);
                    result_publish
                        .send(result_builder.build(JobResultData::Error {
                            path: info.path.clone(),
                            occurred_at: info.path,
                            reason: e.to_string(),
                        }))
                        .expect("Failed to send result");
                    return;
                }
            };

            let mut children = Vec::new();

            for entry in dir {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        error!(
                            "[{id}] Error reading directory entry of {:?}: {e}.",
                            info.path
                        );
                        result_publish
                            .send(result_builder.build(JobResultData::Error {
                                occurred_at: info.path.clone(),
                                path: info.path,
                                reason: e.to_string(),
                            }))
                            .expect("Failed to send result");
                        return;
                    }
                };

                let metadata = match entry.metadata() {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        error!("[{id}] Error reading metadata {:?}: {e}.", entry.path());
                        children.push(DirectoryEntry {
                            path: entry.path(),
                            modified: 0,
                            file_size: 0,
                            file_type: FileType::Other,
                        });
                        continue;
                    }
                };

                children.push(evaluate_file(id, entry.path(), &metadata));
            }

            children.sort_by(|a, b| a.path.cmp(&b.path));

            result_publish
                .send(result_builder.build(JobResultData::DirectoryListing { children, info }))
                .expect("Failed to send result");
        }
        BuildJobData::HashFile(info) => {
            let stream = match File::open(&info.path) {
                Err(err) => {
                    error!("Error while opening file {:?}: {}", info.path, err);
                    result_publish
                        .send(result_builder.build(JobResultData::Error {
                            occurred_at: info.path.clone(),
                            path: info.path,
                            reason: err.to_string(),
                        }))
                        .expect("Failed to send result");
                    return;
                }
                Ok(stream) => stream,
            };
            let mut length_counter = LengthCountingStream::new(stream);
            let stream = BufReader::new(&mut length_counter);

            let mut hasher = HashingStream::new(stream, arg.hash_type);

            let (archive, mut stream) = match arg.archives {
                true => {
                    let stream =
                        BufferCopyStreamReader::with_capacity_compression_peak(&mut hasher);
                    let result = is_archive(stream.child());

                    let stream = stream.try_into_inner().expect("only one instance left");

                    (result, stream)
                }
                false => (
                    Ok(None),
                    BufferCopyStreamReader::with_no_capacity(&mut hasher)
                        .try_into_inner()
                        .expect("only one instance left"),
                ),
            };

            let archive_contents = {
                match archive {
                    Err(err) => Err(anyhow!("Error while probing file for archive: {}", err)),
                    Ok(None) => Ok(None),
                    Ok(Some((compression_type, archive_type))) => {
                        let stream = compression_type.open(&mut stream);
                        Ok(Some(worker_run_archive(
                            stream,
                            &FilePath::from_realpath(&info.path),
                            archive_type,
                            &mut WorkerRunArchiveArguments { id, arg },
                        )))
                    }
                }
            };

            let archive_contents = match archive_contents {
                Err(err) => {
                    error!(
                        "[{}] Error while probing file {:?} for archive: {}",
                        id, info.path, err
                    );
                    result_publish
                        .send(result_builder.build(JobResultData::Error {
                            occurred_at: info.path.clone(),
                            path: info.path,
                            reason: err.to_string(),
                        }))
                        .expect("Failed to send result");
                    return;
                }
                Ok(content) => content,
            };

            if let Err(err) = stream.read_to_end_discarding() {
                error!(
                    "[{id}] Error while reading file {:?}: {err}. Skipping.",
                    info.path
                );
                result_publish
                    .send(result_builder.build(JobResultData::Error {
                        occurred_at: info.path.clone(),
                        path: info.path,
                        reason: err.to_string(),
                    }))
                    .expect("Failed to send result");
                return;
            }
            let hash = hasher.hash();
            let size = length_counter.length();

            let file = match archive_contents {
                None => JobResultData::FileHash { hash, info, size },
                Some(archive_result) => match archive_result {
                    Err(err) => {
                        debug!("[{id}] Error while processing archive {:?}: {err}. Regarding as non-archive.", info.path);
                        JobResultData::FileHash { hash, info, size }
                    }
                    Ok(mut archive_result) => {
                        let directory_hash = {
                            archive_result.sort_by(|a, b| {
                                a.get_inner_hash()
                                    .partial_cmp(b.get_inner_hash())
                                    .expect("Two same hashes must compare to each other")
                            });
                            let mut hash = GeneralHash::from_type(arg.hash_type);
                            let _ = hash.hash_directory_build_files(
                                archive_result.iter().map(|x| x.get_inner_hash()),
                            );
                            hash
                        };
                        JobResultData::ArchiveHash {
                            children: archive_result,
                            content_directory_hash: directory_hash,
                            file_hash: hash,
                            size,
                            info,
                        }
                    }
                },
            };

            result_publish
                .send(result_builder.build(file))
                .expect("Failed to send result");
        }
        BuildJobData::HashDirectory { info, children } => {
            let mut sorted = children.clone();
            sorted.sort_by(|a, b| {
                a.partial_cmp(b)
                    .expect("Two hashes must compare to each other")
            });
            let mut hash = GeneralHash::from_type(arg.hash_type);
            let _ = hash.hash_directory(sorted.iter());

            result_publish
                .send(result_builder.build(JobResultData::DirectoryHash {
                    info,
                    hash,
                    children,
                }))
                .expect("Failed to send result");
        }
        BuildJobData::HashSymlink(info) => {
            let mut hash = GeneralHash::from_type(arg.hash_type);
            hash.hash_path(&info.path);

            result_publish
                .send(result_builder.build(JobResultData::SymlinkHash { hash, info }))
                .expect("Failed to send result");
        }
        BuildJobData::Initial(path) => {
            let metadata = match path.metadata() {
                Ok(metadata) => metadata,
                Err(e) => {
                    error!("[{id}] Error reading metadata from {path:?}: {e}.");
                    result_publish
                        .send(result_builder.build(JobResultData::Error {
                            occurred_at: path.clone(),
                            path,
                            reason: e.to_string(),
                        }))
                        .expect("Failed to send result");
                    return;
                }
            };

            let file = evaluate_file(id, path.clone(), &metadata);

            result_publish
                .send(result_builder.build(JobResultData::InitialEvaluation { info: file }))
                .expect("Failed to send result");
        }
        BuildJobData::DirectoryStub(info) => {
            error!("[{id}] Received unexpected DirectoryStub job. This should not have happened.");
            result_publish
                .send(result_builder.build(JobResultData::Error {
                    occurred_at: info.path.clone(),
                    path: info.path,
                    reason: "Unexpected DirectoryStub job".to_string(),
                }))
                .expect("Failed to send result");
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

pub trait ReadDiscard<const BUFFER_SIZE: usize = 8192>: Read {
    fn read_to_end_discarding(&mut self) -> std::io::Result<u64> {
        let mut total_bytes = 0;
        let mut buffer = [0u8; BUFFER_SIZE];
        loop {
            match self.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => total_bytes += n as u64,
                Err(e) => return Err(e),
            }
        }
        Ok(total_bytes)
    }
}

impl<T> ReadDiscard for T where T: Read {}

pub struct LengthCountingStream<R: Read> {
    inner: R,
    length: u64,
}

impl<R: Read> LengthCountingStream<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, length: 0 }
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for LengthCountingStream<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;
        self.length += bytes_read as u64;
        Ok(bytes_read)
    }
}
