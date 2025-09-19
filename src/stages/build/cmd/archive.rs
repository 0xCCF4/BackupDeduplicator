use crate::archive::{ArchiveEntry, ArchiveType};
use crate::copy_stream::BufferCopyStreamReader;
use crate::hash::{GeneralHash, GeneralHashType, HashingStream};
use crate::path::FilePath;
use crate::stages::build::cmd::worker::{is_archive, WorkerArgument};
use crate::stages::build::output::{HashTreeFileEntry, HashTreeFileEntryType};
use anyhow::{anyhow, Result};
use log::{error, trace, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
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
) -> Result<Vec<ArchiveFile>> {
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

    let root = build_tree_from_list(arguments.arg.hash_type, entries, &root_path);

    Ok(root)
}

fn worker_run_entry<R: Read>(
    root_path: &FilePath,
    mut entry: ArchiveEntry<R>,
    context: &mut WorkerRunArchiveArguments,
) -> Result<ArchiveFile> {
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
            a.get_inner_hash()
                .partial_cmp(b.get_inner_hash())
                .expect("Two hashes must compare to each other")
        });
        let mut hash = GeneralHash::from_type(context.arg.hash_type);
        let _ = hash.hash_directory_build_files(children.iter().map(|x| x.get_inner_hash()));
        hash
    };

    if !is_sub_archive {
        Ok(ArchiveFile::File {
            path: unique_path,
            content_hash: file_hash,
            content_size,
            modified: entry.modified(),
        })
    } else {
        Ok(ArchiveFile::ArchiveFile {
            path: unique_path,
            directory_hash,
            file_hash,
            content_size,
            children,
            modified: entry.modified(),
        })
    }
}

/// recursive insert the target file into the existing tree root elements given in root_set
fn insert_into_tree(
    root_set: &mut Vec<ArchiveFile>,
    target: &ArchiveFile,
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
            if let ArchiveFile::Directory { children, path, .. } = root {
                if let Some(Some(component)) = path.last_component().map(|p| p.iter().last()) {
                    if component == *next_component {
                        let inserted =
                            insert_into_tree(children, target, &path_prefix.join(component));
                        if inserted {
                            return true;
                        } else {
                            // insert in place
                            children.push(target.to_owned());
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
            let new_dir = ArchiveFile::Directory {
                path: value.clone(),
                modified,
                content_hash: GeneralHash::NULL,
                children: vec![last.to_owned()],
                number_of_children: 1,
            };

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
    entries: Vec<ArchiveFile>,
    path_prefix: &FilePath,
) -> Vec<ArchiveFile> {
    let mut root_set = Vec::with_capacity(1);

    let entries: Vec<ArchiveFile> = entries
        .into_iter()
        .map(|value| {
            if let ArchiveFile::File {
                path,
                modified,
                content_hash,
                content_size,
            } = &value
            {
                if *content_size == 0 {
                    ArchiveFile::Directory {
                        path: path.clone(),
                        modified: *modified,
                        children: Vec::default(),
                        number_of_children: 0,
                        content_hash: content_hash.clone(),
                    }
                } else {
                    value
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
        calculate_dir_hash_recursive(hash_type, root);
    }

    root_set
}

/// calculate the hash of directory
fn calculate_dir_hash(hash_type: GeneralHashType, files: &mut [ArchiveFile]) -> GeneralHash {
    files.sort_by(|a, b| {
        a.get_inner_hash()
            .partial_cmp(b.get_inner_hash())
            .expect("Two hashes must compare to each other")
    });

    let mut hash = GeneralHash::from_type(hash_type);
    let _ = hash.hash_directory_build_files(files.iter().map(|x| x.get_inner_hash()));
    hash
}

/// calculate the dir hashes of the archive
fn calculate_dir_hash_recursive(hash_type: GeneralHashType, file: &mut ArchiveFile) {
    match file {
        ArchiveFile::Directory {
            children,
            number_of_children,
            content_hash,
            ..
        } => {
            *number_of_children = children.len() as u64;
            for child in children.iter_mut() {
                calculate_dir_hash_recursive(hash_type, child);
            }
            let hash = calculate_dir_hash(hash_type, children);
            *content_hash = hash;
        }
        ArchiveFile::ArchiveFile {
            children,
            directory_hash,
            ..
        } => {
            for child in children.iter_mut() {
                calculate_dir_hash_recursive(hash_type, child);
            }
            let hash = calculate_dir_hash(hash_type, children);
            *directory_hash = hash;
        }
        ArchiveFile::File { .. } | ArchiveFile::Other { .. } | ArchiveFile::Symlink { .. } => {
            // do nothing
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchiveFile {
    /// A regular file.
    File {
        /// The path of the file.
        path: FilePath,
        /// The last modification time of the file.
        modified: u64,
        /// The hash of the file content.
        content_hash: GeneralHash,
        /// The size of the file content.
        content_size: u64,
    },
    /// An archive file (special variant of file, including subtree).
    ArchiveFile {
        /// The path of the archive file.
        path: FilePath,
        /// The last modification time of the archive file.
        modified: u64,
        /// The hash of the archive file content.
        file_hash: GeneralHash,
        /// The hash of the archive directory structure. Used to find duplicate file structures
        directory_hash: GeneralHash,
        /// The size of the archive file content.
        content_size: u64,
        /// The children of the archive file.
        children: Vec<ArchiveFile>,
    },
    /// A directory.
    Directory {
        /// The path of the directory.
        path: FilePath,
        /// The last modification time of the directory.
        modified: u64,
        /// The hash of the directory content.
        content_hash: GeneralHash,
        /// The number of children in the directory.
        number_of_children: u64,
        /// The children of the directory.
        children: Vec<ArchiveFile>,
    },
    /// A symlink.
    Symlink {
        /// The path of the symlink.
        path: FilePath,
        /// The last modification time of the symlink.
        modified: u64,
        /// The hash of the symlink content.
        content_hash: GeneralHash,
        /// The target of the symlink.
        target: PathBuf,
        /// The size of the symlink content.
        content_size: u64,
    },
    /// A file that is not a regular file, directory, or symlink, or a file for which permissions are missing.
    Other {
        /// The path of the file.
        path: FilePath,
        /// The last modification time of the file.
        modified: u64,
        /// The size of the file content.
        content_size: u64,
    },
}

impl ArchiveFile {
    /// Get the hash of a file
    ///
    /// # Returns
    /// The hash of the file. If the file is of type `Other` the hash is [GeneralHash::NULL].
    pub fn get_file_hash(&self) -> &GeneralHash {
        match self {
            ArchiveFile::File { content_hash, .. } => content_hash,
            ArchiveFile::ArchiveFile { file_hash, .. } => file_hash,
            ArchiveFile::Directory { content_hash, .. } => content_hash,
            ArchiveFile::Symlink { content_hash, .. } => content_hash,
            ArchiveFile::Other { .. } => &GeneralHash::NULL,
        }
    }

    /// Get the hash of the file content or directory structure for archive files
    ///
    /// # Returns
    /// The content hash of the file or directory hash of the archive file. If the file
    /// is of type `Other` the hash is [GeneralHash::NULL].
    pub fn get_inner_hash(&self) -> &GeneralHash {
        match self {
            ArchiveFile::File { content_hash, .. } => content_hash,
            ArchiveFile::ArchiveFile { directory_hash, .. } => directory_hash,
            ArchiveFile::Directory { content_hash, .. } => content_hash,
            ArchiveFile::Symlink { content_hash, .. } => content_hash,
            ArchiveFile::Other { .. } => &GeneralHash::NULL,
        }
    }

    /// Gets the path of this file
    ///
    /// # Returns
    /// The path of the file.
    pub fn get_path(&self) -> &FilePath {
        match self {
            ArchiveFile::File { path, .. } => path,
            ArchiveFile::ArchiveFile { path, .. } => path,
            ArchiveFile::Directory { path, .. } => path,
            ArchiveFile::Symlink { path, .. } => path,
            ArchiveFile::Other { path, .. } => path,
        }
    }

    /// Returns true if this is a directory
    ///
    /// # Returns
    /// True if this is a directory, false otherwise.
    pub fn is_directory(&self) -> bool {
        matches!(self, ArchiveFile::Directory { .. })
    }

    /// Returns true if this is a symlink
    ///
    /// # Returns
    /// True if this is a symlink, false otherwise.
    pub fn is_symlink(&self) -> bool {
        matches!(self, ArchiveFile::Symlink { .. })
    }

    /// Returns true if this is a file
    ///
    /// # Returns
    /// True if this is a file, false otherwise.
    pub fn is_file(&self) -> bool {
        matches!(
            self,
            ArchiveFile::File { .. } | ArchiveFile::ArchiveFile { .. }
        )
    }

    /// Returns true if this is an archive file
    ///
    /// # Returns
    /// True if this is an archive file, false otherwise.
    pub fn is_archive(&self) -> bool {
        matches!(self, ArchiveFile::ArchiveFile { .. })
    }

    /// Returns true if this is an "other" file
    ///
    /// # Returns
    /// True if this is an "other" file, false otherwise.
    pub fn is_other(&self) -> bool {
        matches!(self, ArchiveFile::Other { .. })
    }

    /// Get the last modification time of the file
    ///
    /// # Returns
    /// The last modification time of the file.
    pub fn modified(&self) -> u64 {
        match self {
            ArchiveFile::File { modified, .. } => *modified,
            ArchiveFile::ArchiveFile { modified, .. } => *modified,
            ArchiveFile::Directory { modified, .. } => *modified,
            ArchiveFile::Symlink { modified, .. } => *modified,
            ArchiveFile::Other { modified, .. } => *modified,
        }
    }

    pub fn to_hash_file_entry(&self) -> Vec<HashTreeFileEntry> {
        let mut queue = VecDeque::new();
        queue.push_back(self);
        let mut result = Vec::new();

        while let Some(file) = queue.pop_front() {
            match file {
                ArchiveFile::File {
                    path,
                    modified,
                    content_hash,
                    content_size,
                } => result.push(HashTreeFileEntry {
                    archive_children: vec![],
                    modified: *modified,
                    path: path.clone(),
                    hash: content_hash.clone(),
                    file_type: HashTreeFileEntryType::File,
                    children: vec![],
                    archive_inner_hash: None,
                    size: *content_size,
                }),
                ArchiveFile::Other {
                    path,
                    content_size,
                    modified,
                } => result.push(HashTreeFileEntry {
                    archive_children: vec![],
                    modified: *modified,
                    path: path.clone(),
                    hash: GeneralHash::NULL,
                    file_type: HashTreeFileEntryType::Other,
                    children: vec![],
                    archive_inner_hash: None,
                    size: *content_size,
                }),
                ArchiveFile::Symlink {
                    path,
                    modified,
                    content_hash,
                    target: _,
                    content_size,
                } => result.push(HashTreeFileEntry {
                    archive_children: vec![],
                    modified: *modified,
                    path: path.clone(),
                    hash: content_hash.clone(),
                    file_type: HashTreeFileEntryType::Symlink,
                    children: vec![],
                    archive_inner_hash: None,
                    size: *content_size,
                }),
                ArchiveFile::Directory {
                    path,
                    modified,
                    content_hash,
                    children,
                    number_of_children: _,
                } => {
                    let mut child_entries = Vec::with_capacity(children.len());
                    for child in children {
                        queue.push_back(child);
                        child_entries.push(child.get_inner_hash().clone());
                    }
                    result.push(HashTreeFileEntry {
                        archive_children: vec![],
                        modified: *modified,
                        path: path.clone(),
                        hash: content_hash.clone(),
                        file_type: HashTreeFileEntryType::Directory,
                        children: child_entries,
                        archive_inner_hash: None,
                        size: children.len() as u64,
                    });
                }
                ArchiveFile::ArchiveFile {
                    path,
                    modified,
                    file_hash,
                    directory_hash,
                    children,
                    content_size,
                } => {
                    let mut child_entries = Vec::with_capacity(children.len());
                    for child in children {
                        queue.push_back(child);
                        child_entries.push(child.get_inner_hash().clone());
                    }
                    result.push(HashTreeFileEntry {
                        archive_children: child_entries,
                        modified: *modified,
                        path: path.clone(),
                        hash: file_hash.clone(),
                        file_type: HashTreeFileEntryType::File,
                        children: vec![],
                        archive_inner_hash: Some(directory_hash.clone()),
                        size: *content_size,
                    });
                }
            }
        }

        result
    }
}
