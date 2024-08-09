use crate::hash::GeneralHash;
use crate::path::FilePath;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;

/// Information about an analyzed file.
///
/// # Fields
/// * `path` - The path of the file.
/// * `modified` - The last modification time of the file.
/// * `content_hash` - The hash of the file content.
/// * `content_size` - The size of the file content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildFileInformation {
    /// The path of the file.
    pub path: FilePath,
    /// The last modification time of the file.
    pub modified: u64,
    /// The hash of the file content.
    pub content_hash: GeneralHash,
    /// The size of the file content.
    pub content_size: u64,
}

/// Information about an analyzed archive file.
///
/// # Fields
/// * `path` - The path of the archive file.
/// * `modified` - The last modification time of the archive file.
/// * `file_hash` - The hash of the archive file content.
/// * `directory_hash` - The hash of the archive directory structure. Used to find duplicate file structures
/// * `content_size` - The size of the archive file content.
/// * `children` - The children of the archive file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildArchiveFileInformation {
    /// The path of the archive file.
    pub path: FilePath,
    /// The last modification time of the archive file.
    pub modified: u64,
    /// The hash of the archive file content.
    pub file_hash: GeneralHash,
    /// The hash of the archive directory structure. Used to find duplicate file structures
    pub directory_hash: GeneralHash,
    /// The size of the archive file content.
    pub content_size: u64,
    /// The children of the archive file.
    pub children: Vec<BuildFile>,
}

/// Information about an analyzed directory.
///
/// # Fields
/// * `path` - The path of the directory.
/// * `modified` - The last modification time of the directory.
/// * `content_hash` - The hash of the directory content.
/// * `number_of_children` - The number of children in the directory.
/// * `children` - The children of the directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildDirectoryInformation {
    /// The path of the directory.
    pub path: FilePath,
    /// The last modification time of the directory.
    pub modified: u64,
    /// The hash of the directory content.
    pub content_hash: GeneralHash,
    /// The number of children in the directory.
    pub number_of_children: u64,
    /// The children of the directory.
    pub children: Vec<BuildFile>,
}

impl BuildDirectoryInformation {
    /// Returns the child matching the provided name
    ///
    /// # Arguments
    /// * `name` - The name of the child to find.
    /// * `prefer_directory` - If true, prefer directories to files.
    ///
    /// # Returns
    /// The child matching the provided name. If no child is found, None is returned.
    pub fn get_child_by_name<Str: Into<OsString>>(
        &self,
        name: Str,
        prefer_directory: bool,
    ) -> Option<&BuildFile> {
        let target_path = self.path.join(name);
        let mut result = None;
        for child in self
            .children
            .iter()
            .filter(|child| child.get_path() == &target_path)
        {
            if child.is_directory() && prefer_directory {
                return Some(child);
            }

            if child.is_file() && !prefer_directory {
                return Some(child);
            }

            result = Some(child);
        }
        result
    }

    /// Returns child matching the provided name as mutable
    ///
    /// # Arguments
    /// * `name` - The name of the child to find.
    /// * `prefer_directory` - If true, prefer directories to files.
    ///
    /// # Returns
    /// The child matching the provided name as mutable. If no child is found, None is returned.
    pub fn get_child_by_name_mut<Str: Into<OsString>>(
        &mut self,
        name: Str,
        prefer_directory: bool,
    ) -> Option<&mut BuildFile> {
        let target_path = self.path.join(name);
        let mut result = None;
        for child in self
            .children
            .iter_mut()
            .filter(|child| child.get_path() == &target_path)
        {
            if child.is_directory() && prefer_directory {
                return Some(child);
            }

            if child.is_file() && !prefer_directory {
                return Some(child);
            }

            result = Some(child);
        }
        result
    }
}

/// Information about an analyzed symlink.
///
/// # Fields
/// * `path` - The path of the symlink.
/// * `modified` - The last modification time of the symlink.
/// * `content_hash` - The hash of the symlink content.
/// * `target` - The target of the symlink.
/// * `content_size` - The size of the symlink content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSymlinkInformation {
    /// The path of the symlink.
    pub path: FilePath,
    /// The last modification time of the symlink.
    pub modified: u64,
    /// The hash of the symlink content.
    pub content_hash: GeneralHash, // equal to the target file's hash or if not following symlinks, the symlink's path hashed
    /// The target of the symlink.
    pub target: PathBuf,
    /// The size of the symlink content.
    pub content_size: u64,
}

/// Information about an analyzed file that is not a regular file, directory, or symlink.
/// This could be sockets, block devices, character devices, etc. or file for which permissions are missing.
///
/// # Fields
/// * `path` - The path of the file.
/// * `modified` - The last modification time of the file.
/// * `content_size` - The size of the file content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildOtherInformation {
    /// The path of the file.
    pub path: FilePath,
    /// The last modification time of the file.
    pub modified: u64,
    /// The size of the file content.
    pub content_size: u64,
}

/// Information about a file that is not kept in memory but saved to disk.
///
/// # Fields
/// * `path` - The path of the file.
/// * `content_hash` - The hash of the file content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStubInformation {
    /// The path of the file.
    pub path: FilePath,
    /// The hash of the file content.
    pub content_hash: GeneralHash,
}

/// A file that has been analyzed.
///
/// # Variants
/// * `File` - A regular file.
/// * `ArchiveFile` - An archive file (special variant of file, including subtree).
/// * `Directory` - A directory.
/// * `Symlink` - A symlink.
/// * `Other` - A file that is not a regular file, directory, or symlink, or a file for which permissions are missing.
/// * `Stub` - A file that is not kept in memory but already saved to disk in the hashtree file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildFile {
    /// A regular file.
    File(BuildFileInformation),
    /// An archive file (special variant of file, including subtree).
    ArchiveFile(BuildArchiveFileInformation),
    /// A directory.
    Directory(BuildDirectoryInformation),
    /// A symlink.
    Symlink(BuildSymlinkInformation),
    /// A file that is not a regular file, directory, or symlink, or a file for which permissions are missing.
    Other(BuildOtherInformation), // for unsupported file types like block devices, character devices, etc., or files without permission
    /// A file that is not kept in memory but already saved to disk in the hashtree file.
    Stub(BuildStubInformation), // for files that are already analyzed
}

// ---- IMPLEMENTATION ----

impl BuildFile {
    /// Get the hash of a file
    ///
    /// # Returns
    /// The hash of the file. If the file is of type `Other` the hash is [GeneralHash::NULL].
    pub fn get_content_hash(&self) -> &GeneralHash {
        match self {
            BuildFile::File(info) => &info.content_hash,
            BuildFile::ArchiveFile(info) => &info.directory_hash,
            BuildFile::Directory(info) => &info.content_hash,
            BuildFile::Symlink(info) => &info.content_hash,
            BuildFile::Other(_) => &GeneralHash::NULL,
            BuildFile::Stub(info) => &info.content_hash,
        }
    }

    /// Gets the path of this file
    ///
    /// # Returns
    /// The path of the file.
    pub fn get_path(&self) -> &FilePath {
        match self {
            BuildFile::File(info) => &info.path,
            BuildFile::ArchiveFile(info) => &info.path,
            BuildFile::Directory(info) => &info.path,
            BuildFile::Symlink(info) => &info.path,
            BuildFile::Other(info) => &info.path,
            BuildFile::Stub(info) => &info.path,
        }
    }

    /// Returns true if this is a directory
    ///
    /// # Returns
    /// True if this is a directory, false otherwise.
    pub fn is_directory(&self) -> bool {
        matches!(self, BuildFile::Directory(_))
    }

    /// Returns true if this is a symlink
    ///
    /// # Returns
    /// True if this is a symlink, false otherwise.
    pub fn is_symlink(&self) -> bool {
        matches!(self, BuildFile::Symlink(_))
    }

    /// Returns true if this is a file
    ///
    /// # Returns
    /// True if this is a file, false otherwise.
    pub fn is_file(&self) -> bool {
        matches!(self, BuildFile::File(_) | BuildFile::ArchiveFile(_))
    }

    /// Returns true if this is an archive file
    ///
    /// # Returns
    /// True if this is an archive file, false otherwise.
    pub fn is_archive(&self) -> bool {
        matches!(self, BuildFile::ArchiveFile(_))
    }

    /// Returns true if this is an "other" file
    ///
    /// # Returns
    /// True if this is an "other" file, false otherwise.
    pub fn is_other(&self) -> bool {
        matches!(self, BuildFile::Other(_))
    }

    /// Returns true if this is a stub file
    ///
    /// # Returns
    /// True if this is a stub file, false otherwise.
    pub fn is_stub(&self) -> bool {
        matches!(self, BuildFile::Stub(_))
    }

    /// Get the last modification time of the file
    ///
    /// # Returns
    /// The last modification time of the file.
    pub fn modified(&self) -> u64 {
        match self {
            BuildFile::File(info) => info.modified,
            BuildFile::ArchiveFile(info) => info.modified,
            BuildFile::Directory(info) => info.modified,
            BuildFile::Symlink(info) => info.modified,
            BuildFile::Other(info) => info.modified,
            BuildFile::Stub(_) => 0,
        }
    }
}
