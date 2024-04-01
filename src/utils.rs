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
