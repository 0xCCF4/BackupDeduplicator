use crate::archive::{ArchiveEntry, ArchiveType};
use crate::copy_stream::BufferCopyStreamReader;
use crate::hash::{GeneralHash, GeneralHashType, HashingStream};
use crate::path::FilePath;
use crate::stages::build::cmd::worker::file::is_archive;
use crate::stages::build::cmd::worker::WorkerArgument;
use crate::stages::build::intermediary_build_data::{
    BuildArchiveFileInformation, BuildDirectoryInformation, BuildFile, BuildFileInformation,
};
use anyhow::{anyhow, Result};
use log::{error, trace, warn};
use rand::Rng;
use std::ffi::OsStr;
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

    let root = build_tree_from_list(arguments.arg.hash_type, entries, &root_path)?;

    Ok(root)
}

fn worker_run_entry<R: Read>(
    root_path: &FilePath,
    mut entry: ArchiveEntry<R>,
    context: &mut WorkerRunArchiveArguments,
) -> Result<BuildFile> {
    let path = match entry.path() {
        Ok(path) => path,
        Err(err) => {
            debug!(
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

    let unique_path = root_path.join(path);

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
            content_hash: file_hash,
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

/// recursive insert the target file into the existing tree root elements given in root_set
fn insert_into_tree(
    root_set: &mut Vec<BuildFile>,
    target: &BuildFile,
    path_prefix: &FilePath,
) -> bool {
    let relative_path = {
        let relative = target.get_path().relative_to_last(path_prefix);
        match relative {
            None => {
                warn!(
                    "Could not calculate relative path for {:?} and {:?}",
                    path_prefix,
                    target.get_path()
                );
                return false;
            }
            Some(relative) => relative,
        }
    };

    let target_path_str: Vec<&OsStr> = relative_path.last_component().unwrap().iter().collect();
    let mut target_paths: Vec<FilePath> = Vec::with_capacity(target_path_str.len());
    for path in target_path_str.iter() {
        let last = target_paths.last();
        match last {
            None => target_paths.push(path_prefix.join(path)),
            Some(last) => target_paths.push(last.join(path)),
        }
    }

    // try to add to one of the existing tree
    for root in root_set.iter_mut() {
        let mut target_path_components = target_path_str.iter();
        let next_component = target_path_components.next();
        if let Some(next_component) = next_component {
            if let BuildFile::Directory(dir_content) = root {
                if let Some(Some(component)) =
                    dir_content.path.last_component().map(|p| p.iter().last())
                {
                    if component == *next_component {
                        let inserted = insert_into_tree(
                            &mut dir_content.children,
                            target,
                            &path_prefix.join(component),
                        );
                        if inserted {
                            return true;
                        } else {
                            // insert in place
                            dir_content.children.push(target.to_owned());
                            return true;
                        }
                    }
                }
            }
        }
    }

    // insert as new root tree

    let new_node = if target_paths.len() > 1 {
        let modified = target.modified();
        let mut last = target.to_owned();
        for value in target_paths[..target_paths.len() - 1].iter().rev() {
            let new_dir = BuildFile::Directory(BuildDirectoryInformation {
                path: value.clone(),
                modified,
                content_hash: GeneralHash::NULL,
                children: vec![last.to_owned()],
                number_of_children: 1,
            });

            last = new_dir;
        }
        last
    } else {
        target.to_owned()
    };
    root_set.push(new_node);

    true
}

/// given a list of archive elements builds a tree like structure of the file system in the archive
fn build_tree_from_list(
    hash_type: GeneralHashType,
    entries: Vec<BuildFile>,
    path_prefix: &FilePath,
) -> Result<Vec<BuildFile>> {
    let mut root_set = Vec::with_capacity(1);

    let entries: Vec<BuildFile> = entries
        .into_iter()
        .map(|value| {
            if let BuildFile::File(file_info) = value {
                if file_info.content_size == 0 {
                    BuildFile::Directory(BuildDirectoryInformation {
                        path: file_info.path.clone(),
                        modified: file_info.modified,
                        children: Vec::default(),
                        number_of_children: 0,
                        content_hash: file_info.content_hash.clone(),
                    })
                } else {
                    BuildFile::File(file_info)
                }
            } else {
                value
            }
        })
        .collect();

    for entry in entries {
        let inserted = insert_into_tree(&mut root_set, &entry, path_prefix);
        if !inserted {
            error!("Failed to insert entry into tree: {:?}", entry.get_path());
        }
    }

    // calculate directory hashes
    for root in &mut root_set {
        calculate_dir_hash_recursive(hash_type, root)?;
    }

    Ok(root_set)
}

/// calculate the hash of directory
fn calculate_dir_hash(hash_type: GeneralHashType, files: &mut [BuildFile]) -> Result<GeneralHash> {
    files.sort_by(|a, b| {
        a.get_content_hash()
            .partial_cmp(b.get_content_hash())
            .expect("Two hashes must compare to each other")
    });

    let mut hash = GeneralHash::from_type(hash_type);
    let _ = hash.hash_directory(files.iter())?;
    Ok(hash)
}

/// calculate the dir hashes of the archive
fn calculate_dir_hash_recursive(hash_type: GeneralHashType, file: &mut BuildFile) -> Result<()> {
    match file {
        BuildFile::Directory(dir) => {
            dir.number_of_children = dir.children.len() as u64;
            for child in dir.children.iter_mut() {
                calculate_dir_hash_recursive(hash_type, child)?;
            }
            let hash = calculate_dir_hash(hash_type, &mut dir.children)?;
            dir.content_hash = hash;
        }
        BuildFile::ArchiveFile(archive) => {
            for child in archive.children.iter_mut() {
                calculate_dir_hash_recursive(hash_type, child)?;
            }
            let hash = calculate_dir_hash(hash_type, &mut archive.children)?;
            archive.directory_hash = hash;
        }
        BuildFile::File(_) | BuildFile::Other(_) | BuildFile::Stub(_) | BuildFile::Symlink(_) => {
            // do nothing
        }
    }
    Ok(())
}
