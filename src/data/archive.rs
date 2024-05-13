use std::fmt::{Debug, Formatter};
use std::io::Read;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use anyhow::{Result};
use crate::utils;

/// The type of archive.
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum ArchiveType {
    #[cfg(feature = "archive-tar")]
    Tar,
}

impl ArchiveType {
    /// Open an archive stream.
    /// 
    /// # Arguments
    /// * `stream` - The stream to open.
    /// 
    /// # Returns
    /// An iterator over the archive entries.
    /// 
    /// # Errors
    /// If the archive could not be opened.
    pub fn open<R: Read + 'static>(&self, stream: R) -> Result<Box<dyn Iterator<Item=Result<ArchiveEntry>>>> {
        match self {
            #[cfg(feature = "archive-tar")]
            ArchiveType::Tar => Ok(Box::new(tar::TarArchiveIterator::new(stream)?) as Box<dyn Iterator<Item = Result<ArchiveEntry, anyhow::Error>>>),
        }
    }
    
    /// Get the archive type from the file extension.
    /// 
    /// # Arguments
    /// * `extension` - The file extension.
    /// 
    /// # Returns
    /// The archive type. None if the extension is not recognized.
    pub fn from_extension(extension: &str) -> Option<ArchiveType> {
        match extension {
            #[cfg(feature = "archive-tar")]
            "tar" => Some(ArchiveType::Tar),
            _ => None,
        }
    }

    /// Get the maximum amount of bytes to peek from the stream to determine the archive type.
    /// 
    /// # Returns
    /// The maximum amount of bytes to peek needed to determine the archive type.
    pub const fn max_stream_peek_count() -> usize {
        const MAX_BYTES_TAR: usize = match cfg!(feature = "archive-tar") {
            true => 257 + 8,
            false => 0
        };
        const MAX_BYTES: usize = utils::max(MAX_BYTES_TAR, 0);

        MAX_BYTES
    }

    /// Get the archive type from the stream.
    /// 
    /// # Arguments
    /// * `stream` - The stream to read from.
    /// 
    /// # Returns
    /// The archive type. None if the archive type could not be determined.
    /// 
    /// # Errors
    /// If the stream could not be read.
    pub fn from_stream<R: Read>(stream: R) -> Result<Option<ArchiveType>> {
        const MAX_BYTES: usize = ArchiveType::max_stream_peek_count();

        let mut buffer = [0; MAX_BYTES];

        let mut stream = stream.take(MAX_BYTES as u64);

        let mut num_read_sum: usize = 0;
        while stream.limit() > 0 {
            let num_read = stream.read(&mut buffer[num_read_sum..])?;
            num_read_sum += num_read;
            if num_read <= 0 {
                break;
            }
        }

        #[cfg(feature = "archive-tar")]
        {
            if num_read_sum >= 257 + 8 &&
                buffer[257+0] == 0x75 &&
                buffer[257+1] == 0x73 &&
                buffer[257+2] == 0x74 &&
                buffer[257+3] == 0x61 &&
                buffer[257+4] == 0x72 &&
                buffer[257+5] == 0x00 &&
                buffer[257+6] == 0x30 &&
                buffer[257+7] == 0x30 {
                return Ok(Some(ArchiveType::Tar));
            }

            if num_read_sum >= 257 + 8 &&
                buffer[257+0] == 0x75 &&
                buffer[257+1] == 0x73 &&
                buffer[257+2] == 0x74 &&
                buffer[257+3] == 0x61 &&
                buffer[257+4] == 0x72 &&
                buffer[257+5] == 0x20 &&
                buffer[257+6] == 0x20 &&
                buffer[257+7] == 0x00 {
                return Ok(Some(ArchiveType::Tar));
            }
            
            if num_read_sum >= 8 &&
                buffer[0] == 0x75 &&
                buffer[1] == 0x73 &&
                buffer[2] == 0x74 &&
                buffer[3] == 0x61 &&
                buffer[4] == 0x72 &&
                buffer[5] == 0x20 &&
                buffer[6] == 0x20 &&
                buffer[7] == 0x00 {
                return Ok(Some(ArchiveType::Tar));
            }
        }
        
        return Ok(None);
    }
}

/// An entry in an archive.
/// 
/// # Fields
/// * `path` - The path of the entry.
/// * `size` - The size of the entry.
/// * `modified` - The last modified time of the entry.
/// * `stream` - The stream of the entry.
pub struct ArchiveEntry {
    pub path: PathBuf,
    pub size: u64,
    pub modified: u64,
    pub stream: Box<dyn Read>,
}

impl Debug for ArchiveEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArchiveEntry {{ path: {:?}, size: {}, modified: {} }}", self.path, self.size, self.modified)
    }
}

#[cfg(feature = "archive-tar")]
pub mod tar;
