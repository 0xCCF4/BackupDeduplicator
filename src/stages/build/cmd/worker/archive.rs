use crate::archive::{ArchiveEntry, ArchiveType};
use crate::copy_stream::BufferCopyStreamReader;
use crate::hash::{GeneralHash, HashingStream};
use crate::path::FilePath;
use crate::stages::build::cmd::worker::file::is_archive;
use crate::stages::build::cmd::worker::WorkerArgument;
use crate::stages::build::intermediary_build_data::{
    BuildArchiveFileInformation, BuildFile, BuildFileInformation,
};
use anyhow::{anyhow, Result};
use log::{error, trace, warn};
use rand::Rng;
use std::io::Read;
use std::path::PathBuf;

pub struct WorkerRunArchiveArguments<'a> {
    pub id: usize,
    pub arg: &'a mut WorkerArgument,
}

/// Accepts an uncompressed input stream. .gz streams e.g. must be uncompressed by the caller.
pub fn worker_run_archive<R: Read>(
    mut input: R,
    path: &FilePath,
    archive_type: ArchiveType,
    arguments: &mut WorkerRunArchiveArguments,
) -> Result<Vec<BuildFile>> {
    let mut archive = archive_type
        .open(&mut input)
        .map_err(|err| anyhow!("Failed to open archive: {}", err))?;

    let root_path = path.new_archive();

    let mut entries = Vec::new();

    for entry in archive.entries()? {
        let entry = entry.map_err(|err| anyhow!("Failed to read archive entry: {}", err))?;

        let result = worker_run_entry(&root_path, entry, arguments)?;

        entries.push(result);
    }

    // todo organize entries as tree

    Ok(entries)
}

fn worker_run_entry<R: Read>(
    root_path: &FilePath,
    mut entry: ArchiveEntry<R>,
    context: &mut WorkerRunArchiveArguments,
) -> Result<BuildFile> {
    let path = match entry.path() {
        Ok(path) => path,
        Err(err) => {
            error!(
                "[{}] Error while reading archive entry path: {}",
                context.id, err
            );
            PathBuf::from(
                String::from("bdd_unknown_")
                    + &*rand::thread_rng()
                        .sample_iter(&rand::distributions::Alphanumeric)
                        .map(|value| value as char)
                        .take(10)
                        .collect::<String>(),
            )
        }
    };

    let mut is_sub_archive = false;

    let unique_path = root_path.child(path);

    trace!(
        "[{}] Processing archive entry: {:?}",
        context.id,
        unique_path
    );

    let stream = entry.stream();
    let mut hasher = HashingStream::new(stream, context.arg.hash_type);
    let stream = BufferCopyStreamReader::new(&mut hasher);

    let archive = is_archive(stream.child())?; // stream could not be read

    let mut stream = stream.try_into_inner()?; // should never fail, since child is out of context by now

    let mut children = if let Some((compression, archive)) = archive {
        let uncompressed_stream = compression.open(&mut stream);
        let contents = worker_run_archive(uncompressed_stream, &unique_path, archive, context);

        match contents {
            Err(err) => {
                warn!("[{}] Error while reading nested archive {}: Skipping it. Handling it as normal file: {}", context.id, unique_path, err);
                Vec::default()
            }
            Ok(contents) => {
                is_sub_archive = true;
                contents
            }
        }
    } else {
        Vec::default()
    };

    let _ = std::io::copy(&mut stream, &mut std::io::sink())
        .map_err(|err| anyhow!("Error while hashing file {:?}: {}", unique_path, err))?;
    drop(stream);

    let content_size = hasher.bytes_processed();

    let file_hash = hasher.hash();
    let directory_hash = if !is_sub_archive {
        GeneralHash::NULL
    } else {
        children.sort_by(|a, b| {
            a.get_content_hash()
                .partial_cmp(b.get_content_hash())
                .expect("Two hashes must compare to each other")
        });
        let mut hash = GeneralHash::from_type(context.arg.hash_type);
        let _ = hash.hash_directory(children.iter());
        hash
    };

    if !is_sub_archive {
        Ok(BuildFile::File(BuildFileInformation {
            path: unique_path,
            content_hash: directory_hash,
            content_size,
            modified: entry.modified(),
        }))
    } else {
        Ok(BuildFile::ArchiveFile(BuildArchiveFileInformation {
            path: unique_path,
            directory_hash,
            file_hash,
            content_size,
            children,
            modified: entry.modified(),
        }))
    }
}
