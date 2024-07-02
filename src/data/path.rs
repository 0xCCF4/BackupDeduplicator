use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fmt::Formatter;
use std::path::PathBuf;

/// A path component. A path points to a file or an archive.
///
/// # Fields
/// * `path` - The path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PathComponent {
    /// The path.
    pub path: PathBuf,
}

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
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct FilePath {
    /// The path components.
    pub path: Vec<PathComponent>,
}

impl FilePath {
    /// Creates a new file path from path components.
    ///
    /// # Arguments
    /// * `path` - The path components.
    ///
    /// # Returns
    /// The file path.
    pub fn from_pathcomponents(path: Vec<PathComponent>) -> Self {
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
            path: vec![PathComponent { path: path.into() }],
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

        result.path.push(PathComponent {
            path: PathBuf::from(String::from("")),
        });

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
            Ok(self.path[0].path.clone())
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
    /// let child = path.child("child.txt");
    ///
    /// assert_eq!(child.path[0].path, PathBuf::from("test/child.txt"));
    /// assert_eq!(child.path.len(), 1);
    /// ```
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use backup_deduplicator::path::FilePath;
    ///
    /// let path = FilePath::from_realpath(PathBuf::from("test/"));
    /// let subpath = path.child("subdir").child("abc.txt");
    ///
    /// assert_eq!(subpath.path[0].path, PathBuf::from("test/subdir/abc.txt"));
    /// assert_eq!(subpath.path.len(), 1);
    /// ```
    pub fn child<Str: Into<OsString>>(&self, child_name: Str) -> FilePath {
        let mut result = FilePath {
            path: self.path.clone(),
        };

        let component = PathBuf::from(child_name.into());

        match result.path.last_mut() {
            Some(last) => {
                last.path.push(component);
            }
            None => {
                result.path.push(PathComponent { path: component });
            }
        }

        return result;
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
    /// assert_eq!(parent.path[0].path, PathBuf::from("test/abc"));
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
                let parent = last.path.parent();

                match parent {
                    Some(parent) => {
                        let mut result = FilePath {
                            path: self.path.clone(),
                        };
                        let last = result.path.last_mut().unwrap();
                        last.path = parent.to_path_buf();

                        Some(result)
                    }
                    None => {
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
                }
            }
        }
    }
}

impl PartialEq for FilePath {
    /// Compares two file paths.
    ///
    /// # Arguments
    /// * `other` - The other file path.
    ///
    /// # Returns
    /// Whether the file paths are equal.
    fn eq(&self, other: &Self) -> bool {
        self.path.len() == other.path.len()
            && self.path.iter().zip(other.path.iter()).all(|(a, b)| a == b)
    }
}

impl Eq for FilePath {}

impl std::fmt::Display for FilePath {
    /// Formats the file path to a string.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut result = String::new();

        let mut first = true;
        for component in &self.path {
            if first {
                first = false;
            } else {
                result.push_str("| ");
            }

            result.push_str(component.path.to_str().unwrap_or_else(|| "<invalid path>"));
        }

        write!(f, "{}", result)
    }
}
