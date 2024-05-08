use std::io::{Read, Write};
use std::ops::Deref;
use std::path::{PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Result};

/// Trait to convert a path to a lexical absolute path.
/// Does not require the path to exist.
/// 
/// # See also
/// * <https://internals.rust-lang.org/t/path-to-lexical-absolute/14940>
/// * [std::fs::canonicalize]
pub trait LexicalAbsolute {
    /// Convert a path to a lexical absolute path.
    /// Does not require the path to exist.
    /// 
    /// # Errors
    /// Returns an error if the absolute path could not be determined.
    fn to_lexical_absolute(&self) -> std::io::Result<PathBuf>;
}

impl LexicalAbsolute for PathBuf {
    /// Convert a path to a lexical absolute path.
    /// Does not require the path to exist.
    ///
    /// # Example
    /// ```
    /// use std::path::PathBuf;
    /// use backup_deduplicator::utils::LexicalAbsolute;
    ///
    /// let path = PathBuf::from("/a/b/../c");
    /// let absolute = path.to_lexical_absolute().unwrap();
    /// assert_eq!(absolute, PathBuf::from("/a/c"));
    /// ```
    /// 
    /// # Errors
    /// Returns an error if the given path is relative and the current working directory could not be determined.
    /// * The working directory does not exist.
    /// * Insufficient permissions to determine the working directory.
    fn to_lexical_absolute(&self) -> std::io::Result<PathBuf> {
        // https://internals.rust-lang.org/t/path-to-lexical-absolute/14940
        let mut absolute = if self.is_absolute() {
            PathBuf::new()
        } else {
            std::env::current_dir()?
        };
        for component in self.components() {
            match component {
                std::path::Component::CurDir => {},
                std::path::Component::ParentDir => { absolute.pop(); },
                component @ _ => absolute.push(component.as_os_str()),
            }
        }
        Ok(absolute)
    }
}

/// Decode a hex string to a byte vector.
/// 
/// # Example
/// ```
/// use backup_deduplicator::utils::decode_hex;
/// 
/// let bytes = decode_hex("deadbeef").unwrap();
/// assert_eq!(bytes, vec![0xde, 0xad, 0xbe, 0xef]);
/// ```
/// 
/// # Errors
/// Returns an error if the given string is not a valid hex string.
pub fn decode_hex(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(anyhow!("Invalid hex length"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16)
            .map_err(|e| anyhow!("Failed to parse hex: {}", e)))
        .collect()
}

/// Get the current time in seconds since the Unix epoch (in seconds).
/// 
/// # Returns
/// The current time in seconds since the Unix epoch. Returns 0 if the current time is before the Unix epoch.
pub fn get_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs()).unwrap_or(0)
}

/// A writer that discards all data.
/// 
/// # Example
/// ```
/// use std::io::Write;
/// 
/// let mut writer = backup_deduplicator::utils::NullWriter::new();
/// writer.write(b"Hello, world!").unwrap();
/// ```
pub struct NullWriter {}

impl NullWriter {
    /// Create a new NullWriter.
    /// 
    /// # Returns
    /// A new NullWriter.
    pub fn new() -> Self {
        NullWriter {}
    }
}

impl Write for NullWriter {
    /// Discard all data.
    /// 
    /// # Arguments
    /// * `buf` - The data to write.
    /// 
    /// # Returns
    /// The number of bytes written. Always the same as the length of `buf`.
    /// 
    /// # Errors
    /// Never
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {Ok(buf.len())}

    /// Flush the writer.
    /// 
    /// # Errors
    /// Never
    fn flush(&mut self) -> std::io::Result<()> {Ok(())}
}

/// A reader that does not provide data
/// 
/// # Example
/// ```
/// use std::io::Read;
/// 
/// let mut reader = backup_deduplicator::utils::NullReader::new();
/// let mut buf = [0; 10];
/// assert_eq!(reader.read(&mut buf).unwrap(), 0);
/// ```
pub struct NullReader {}

impl NullReader {
    /// Create a new NullReader.
    /// 
    /// # Returns
    /// A new NullReader.
    pub fn new() -> Self {
        NullReader {}
    }
}

impl std::io::Read for NullReader {
    /// Does not provide any data.
    /// 
    /// # Arguments
    /// * `buf` - The buffer to read into.
    /// 
    /// # Returns
    /// The number of bytes read. Always 0.
    /// 
    /// # Errors
    /// Never
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

// Container that calls a function when value is dropped.
pub struct DestroyContainer<T, F: FnOnce() -> ()> {
    inner: T,
    destroy_func: Option<F>,
}

impl<T, F: FnOnce() -> ()> DestroyContainer<T, F> {
    /// Create a new [DestroyContainer]. The destroy function is called when `this`
    /// instance is dropped.
    ///
    /// # Arguments
    /// * `inner` - The inner value.
    /// * `destroy_func` - The function to call when `this` instance is dropped.
    ///
    /// # Returns
    /// A new [DestroyContainer].
    pub fn new(inner: T, destroy_func: F) -> Self {
        DestroyContainer {
            inner: inner,
            destroy_func: Some(destroy_func),
        }
    }
}

impl <T, F: FnOnce() -> ()> Drop for DestroyContainer<T, F> {
    fn drop(&mut self) {
        self.destroy_func.take().map(|f| f());
    }
}

impl<T, F: FnOnce() -> ()> Deref for DestroyContainer<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: Read, F: FnOnce() -> ()> Read for DestroyContainer<T, F> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Utility functions for the main function of `backup-deduplicator`.
pub mod main {
    use std::env;
    use std::path::PathBuf;
    use crate::utils::LexicalAbsolute;

    /// Changes the working directory to the given path.
    ///
    /// # Arguments
    /// * `working_directory` - The new working directory.
    ///
    /// # Returns
    /// The new working directory.
    ///
    /// # Exit
    /// Exits the process if the working directory could not be changed.
    pub fn change_working_directory(working_directory: Option<PathBuf>) -> PathBuf {
        match working_directory {
            None => {},
            Some(working_directory) => {
                env::set_current_dir(&working_directory).unwrap_or_else(|_| {
                    eprintln!("IO error, could not change working directory: {}", working_directory.display());
                    std::process::exit(exitcode::CONFIG);
                });
            }
        }

        env::current_dir().unwrap_or_else(|_| {
            eprintln!("IO error, could not resolve working directory");
            std::process::exit(exitcode::CONFIG);
        }).canonicalize().unwrap_or_else(|_| {
            eprintln!("IO error, could not resolve working directory");
            std::process::exit(exitcode::CONFIG);
        })
    }

    /// Option how to parse a path.
    ///
    /// # See also
    /// * [parse_path]
    #[derive(Debug, Clone, Copy)]
    pub enum ParsePathKind {
        /// Do not post-process the path.
        Direct,
        /// Convert the path to a absolute path. The path must exist.
        AbsoluteExisting,
        /// Convert the path to a absolute path. The path might not exist.
        AbsoluteNonExisting,
    }

    /// Parse a path from a string.
    ///
    /// # Arguments
    /// * `path` - The path to parse.
    /// * `kind` - How to parse the path.
    ///
    /// # Returns
    /// The parsed path.
    pub fn parse_path(path: &str, kind: ParsePathKind) -> PathBuf {
        let path = std::path::Path::new(path);

        let path = path.to_path_buf();

        let path = match kind {
            ParsePathKind::Direct => path,
            ParsePathKind::AbsoluteExisting => to_lexical_absolute(path, true),
            ParsePathKind::AbsoluteNonExisting => to_lexical_absolute(path, false),
        };

        path
    }

    /// Convert a path to a absolute path.
    ///
    /// # Arguments
    /// * `path` - The path to convert.
    /// * `exists` - Whether the path must exist.
    ///
    /// # Returns
    /// The absolute path.
    ///
    /// # Exit
    /// Exits the process if the path could not be resolved.
    pub fn to_lexical_absolute(path: PathBuf, exists: bool) -> PathBuf {
        let path = match exists {
            true => path.canonicalize(),
            false => path.to_lexical_absolute(),
        };

        let path = match path{
            Ok(out) => out,
            Err(e) => {
                eprintln!("IO error, could not resolve output file: {:?}", e);
                std::process::exit(exitcode::CONFIG);
            }
        };

        path
    }
}