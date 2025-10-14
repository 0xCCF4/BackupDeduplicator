use crate::utils::main::to_relative;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fmt::Formatter;
use std::path::PathBuf;

/// A file path. A file path specifies a target file. It may consist of multiple path components.
/// Imagine the following file structure:
///
/// ```text
/// DIR stuff
/// \-- DIR more_stuff
///   \-- FILE archive.tar.gz
///     \-- FILE file_in_archive.txt
/// ```
///
/// The file path to `file_in_archive.txt` would consist of the following path components:
/// - `stuff/more_stuff/archive.tar.gz`
/// - `file_in_archive.txt`
///
/// The file path to `archive.tar.gz` would consist of the following path components:
/// - `stuff/more_stuff/archive.tar.gz`
///
/// # Fields
/// * `path` - The path components.
///
/// # Examples
/// ```
/// use std::path::PathBuf;
/// use backup_deduplicator::path::FilePath;
///
/// let path = FilePath::from_realpath(PathBuf::from("test.txt"));
///
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, Default, Ord, PartialOrd)]
pub struct FilePath {
    /// The path components.
    pub path: Vec<PathBuf>,
}

impl FilePath {
    /// Creates a new file path from path components.
    ///
    /// # Arguments
    /// * `path` - The path components.
    ///
    /// # Returns
    /// The file path.
    pub fn from_pathcomponents(path: Vec<PathBuf>) -> Self {
        FilePath { path }
    }

    /// Creates a new file path from a real path.
    ///
    /// # Arguments
    /// * `path` - The real path.
    ///
    /// # Returns
    /// The file path.
    pub fn from_realpath<P: Into<PathBuf>>(path: P) -> Self {
        FilePath {
            path: vec![path.into()],
        }
    }

    /// Creates a new subpath from a file path. By starting a nested file path.
    ///
    /// # Returns
    /// The new file path.
    pub fn new_archive(&self) -> FilePath {
        let mut result = FilePath {
            path: self.path.clone(),
        };

        result.path.push(PathBuf::from(String::from("")));

        result
    }

    /// Resolves the file path to a single file.
    ///
    /// # Returns
    /// The resolved file path.
    ///
    /// # Errors
    /// When the file path has multiple components.
    pub fn resolve_file(&self) -> Result<PathBuf> {
        if self.path.len() == 1 {
            Ok(self.path[0].clone())
        } else {
            Err(anyhow::anyhow!(
                "Cannot resolve file path with multiple components"
            ))
        }
    }

    /// Gets the child of where the file path points to.
    ///
    /// # Arguments
    /// * `child_name` - The name of the child.
    ///
    /// # Returns
    /// The child file path.
    ///
    /// # Example
    /// ```
    /// use std::path::PathBuf;
    /// use backup_deduplicator::path::FilePath;
    ///
    /// let path = FilePath::from_realpath(PathBuf::from("test/"));
    /// let child = path.join("child.txt");
    ///
    /// assert_eq!(child.path[0], PathBuf::from("test/child.txt"));
    /// assert_eq!(child.path.len(), 1);
    /// ```
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use backup_deduplicator::path::FilePath;
    ///
    /// let path = FilePath::from_realpath(PathBuf::from("test/"));
    /// let subpath = path.join("subdir").join("abc.txt");
    ///
    /// assert_eq!(subpath.path[0], PathBuf::from("test/subdir/abc.txt"));
    /// assert_eq!(subpath.path.len(), 1);
    /// ```
    pub fn join<Str: Into<OsString>>(&self, child_name: Str) -> FilePath {
        let mut result = FilePath {
            path: self.path.clone(),
        };

        let component = PathBuf::from(child_name.into());

        match result.path.last_mut() {
            Some(last) => {
                last.push(component);
            }
            None => {
                result.path.push(component);
            }
        }

        result
    }

    /// Gets the first component of the file path.
    ///
    /// # Returns
    /// The first component of the file path. None if the file path is empty.
    pub fn first_component(&self) -> Option<&PathBuf> {
        self.path.first()
    }

    /// Gets the first component of the file path.
    ///
    /// # Returns
    /// The first component of the file path. None if the file path is empty.
    pub fn first_component_mut(&mut self) -> Option<&mut PathBuf> {
        self.path.first_mut()
    }

    /// Gets the last component of the file path.
    ///
    /// # Returns
    /// The last component of the file path. None if the file path is empty.
    pub fn last_component(&self) -> Option<&PathBuf> {
        self.path.last()
    }

    /// Gets the last component of the file path.
    ///
    /// # Returns
    /// The last component of the file path. None if the file path is empty.
    pub fn last_component_mut(&mut self) -> Option<&mut PathBuf> {
        self.path.last_mut()
    }

    /// Gets the parent of the file path.
    ///
    /// # Returns
    /// The parent file path. None if the file path has no parent.
    ///
    /// # Example
    /// ```
    /// use std::path::PathBuf;
    /// use backup_deduplicator::path::FilePath;
    ///
    /// let path = FilePath::from_realpath(PathBuf::from("test/abc/def.txt"));
    /// let parent = path.parent().unwrap();
    ///
    /// assert_eq!(parent.path[0], PathBuf::from("test/abc"));
    ///
    /// //                      test/abc          test/             ""        None
    /// let root = path.parent().unwrap().parent().unwrap().parent().unwrap().parent();
    ///
    /// assert_eq!(root, None);
    /// ```
    pub fn parent(&self) -> Option<FilePath> {
        let last = self.path.last();

        match last {
            None => None,
            Some(last) => {
                let parent = last.parent();

                match parent {
                    Some(parent) => {
                        let mut result = FilePath {
                            path: self.path.clone(),
                        };
                        let last = result.path.last_mut().unwrap();
                        *last = parent.to_path_buf();

                        Some(result)
                    }
                    None => self.parent_archive(),
                }
            }
        }
    }

    /// Gets the parent archive path of the file path.
    ///
    /// # Returns
    /// The parent archive file path. None if the file path has no parent archive.
    ///
    /// # Example
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use backup_deduplicator::path::FilePath;
    ///
    /// let path = FilePath::from_realpath(PathBuf::from("test/abc/def.zip")).new_archive().join("dir1").join("file1.txt");
    ///
    /// assert_eq!(path.path[0], PathBuf::from("test/abc/def.zip"));
    /// assert_eq!(path.path[1], PathBuf::from("dir1/file1.txt"));
    /// assert_eq!(path.path.len(), 2);
    ///
    /// let parent_archive = path.parent_archive().unwrap();
    ///
    /// assert_eq!(parent_archive.path[0], PathBuf::from("test/abc/def.zip"));
    /// assert_eq!(parent_archive.path.len(), 1);
    /// ```
    pub fn parent_archive(&self) -> Option<FilePath> {
        if self.path.len() == 1 {
            None
        } else {
            let mut result = FilePath {
                path: self.path.clone(),
            };
            result.path.pop();
            Some(result)
        }
    }

    /// Makes the last component of the file path relative to the last component of the given file path.
    ///
    /// # Arguments
    /// * `absolute_path` - The file path to make this relative to
    ///
    /// # Returns
    /// The relative file path. None if the file paths have no common root.
    ///
    /// # Example
    /// ```
    /// use std::path::PathBuf;
    /// use backup_deduplicator::path::FilePath;
    ///
    /// let path = FilePath::from_realpath(PathBuf::from("test/abc/def.zip"));
    /// let prefix = FilePath::from_realpath(PathBuf::from("test/"));
    /// let relative = path.relative_to_last(&prefix).unwrap();
    ///
    /// assert_eq!(relative.path[0], PathBuf::from("abc/def.zip"));
    /// ```
    pub fn relative_to_last(&self, absolute_path: &FilePath) -> Option<FilePath> {
        if self.parent_archive() != absolute_path.parent_archive() {
            return None; // no common root
        }

        let this = self.last_component();
        let absolute = absolute_path.last_component();

        match (this, absolute) {
            (None, None) => None,
            (Some(_), None) => None,
            (None, Some(absolute)) => Some(
                absolute_path
                    .parent_archive()
                    .unwrap_or_default()
                    .join(absolute),
            ),
            (Some(this), Some(absolute)) => to_relative(this, absolute).map(|value| {
                absolute_path
                    .parent_archive()
                    .unwrap_or_default()
                    .new_archive()
                    .join(value)
            }),
        }
    }

    pub fn len(&self) -> usize {
        self.path.len()
    }
}

impl std::fmt::Display for FilePath {
    /// Formats the file path to a string.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut result = String::new();

        let mut first = true;
        for component in &self.path {
            if first {
                first = false;
            } else {
                result.push_str("->");
            }

            result.push_str(
                component
                    .to_str()
                    .map(|str| str.replace("->", "\\->"))
                    .unwrap_or_else(|| "<invalid path>".to_owned())
                    .as_ref(),
            );
        }

        write!(f, "{}", result)
    }
}
