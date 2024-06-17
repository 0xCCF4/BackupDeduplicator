use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::io::Read;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::rc::Rc;
use serde::{Deserialize, Serialize};
use anyhow::{Result};
use crate::compression::CompressionType;
use crate::copy_stream::BufferCopyStreamReader;
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
    pub fn open<'a, 'b: 'a, R: Read + 'b>(&self, stream: R) -> Result<Box<dyn ArchiveIterator<'b> + 'a>> {
        match self {
            #[cfg(feature = "archive-tar")]
            ArchiveType::Tar => Ok(Box::new(tar::TarArchiveIterator::new(stream)?) as Box<dyn ArchiveIterator + 'a>),
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
    stream: Box<dyn Read>,
}

impl Read for ArchiveEntry {
    /// Read from the archive entry.
    /// 
    /// # Arguments
    /// * `buf` - The buffer to read into.
    /// 
    /// # Returns
    /// The number of bytes read.
    /// 
    /// # Errors
    /// If the entry could not be read.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Borrow_mut should never panic, since there exists only one ArchiveEntry with a reference
        // to the stream at a time.
        self.stream.read(buf)
    }
}

impl<'a> Debug for ArchiveEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArchiveEntry {{ path: {:?}, size: {}, modified: {} }}", self.path, self.size, self.modified)
    }
}

#[cfg(feature = "archive-tar")]
pub mod tar;

impl<R: Read> BufferCopyStreamReader<R> {
    /// Create a new buffer copy stream reader with the capacity to determine the compression type.
    ///
    /// # Arguments
    /// * `stream` - The stream to read from.
    ///
    /// # Returns
    /// The buffer copy stream reader.
    ///
    /// # See also
    /// [BufferCopyStreamReader]
    pub fn with_capacity_archive_peak(stream: R) -> BufferCopyStreamReader<R> {
        BufferCopyStreamReader::with_capacity(stream, ArchiveType::max_stream_peek_count())
    }
}

trait ArchiveIterator<'a> {
    fn next(&mut self) -> Option<Result<()>>;
    fn current_entry(&mut self) -> Option<&mut ArchiveEntry>;
}
