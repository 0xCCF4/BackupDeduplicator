use std::fmt::{Debug, Formatter};
use std::io::Read;
use std::path::{PathBuf};
use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use crate::copy_stream::BufferCopyStreamReader;
use crate::utils;

/// The type of archive.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum ArchiveType {
    #[cfg(feature = "archive-tar")]
    Tar,
    #[cfg(feature = "archive-zip")]
    Zip,
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
    pub fn open<R: Read>(&self, stream: R) -> Result<GeneralArchive<R>> {
        GeneralArchive::new(self.clone(), stream)
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
            #[cfg(feature = "archive-zip")]
            "zip" => Some(ArchiveType::Zip),
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
        const MAX_BYTES_ZIP: usize = match cfg!(feature = "archive-zip") {
            true => 4,
            false => 0
        };
        const MAX_BYTES: usize = utils::max(MAX_BYTES_TAR, MAX_BYTES_ZIP);

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

        #[cfg(feature = "archive-zip")]
        {
            if num_read_sum >= 4 &&
                buffer[0] == 0x50 &&
                buffer[1] == 0x4b &&
                buffer[2] == 0x03 &&
                buffer[3] == 0x04 {
                return Ok(Some(ArchiveType::Zip));
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

pub enum GeneralArchive<R: Read> {
    #[cfg(feature = "archive-tar")]
    Tar(tar::TarArchive<R>),
    #[cfg(feature = "archive-zip")]
    Zip(zip::ZipArchive<R>),
}

impl<R: Read> GeneralArchive<R> {
    pub fn new(archive_type: ArchiveType, stream: R) -> Result<Self> {
        Ok(match archive_type { 
            #[cfg(feature = "archive-tar")]
            ArchiveType::Tar => Self::Tar(tar::TarArchive::new(stream)?),
            #[cfg(feature = "archive-zip")]
            ArchiveType::Zip => Self::Zip(zip::ZipArchive::new(stream.into())?),
        })
    }
    
    pub fn entries(&mut self) -> Result<ArchiveIterator<R>> {
        Ok(match self {
            #[cfg(feature = "archive-tar")]
            Self::Tar(archive) => ArchiveIterator::Tar(archive.entries()?),
            #[cfg(feature = "archive-zip")]
            Self::Zip(archive) => ArchiveIterator::Zip(archive.entries()?),
        })
    }
}

pub enum ArchiveIterator<'a, R: Read> {
    #[cfg(feature = "archive-tar")]
    Tar(tar::TarArchiveIterator<'a, R>),
    #[cfg(feature = "archive-zip")]
    Zip(zip::ZipArchiveIterator<'a, R>),
}

impl<'a, R: Read> Iterator for ArchiveIterator<'a, R> {
    type Item = Result<ArchiveEntry<'a, R>>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            #[cfg(feature = "archive-tar")]
            Self::Tar(iterator) => iterator.next(),
            #[cfg(feature = "archive-zip")]
            Self::Zip(iterator) => iterator.next(),
        }
    }
}

pub enum ArchiveEntry<'a, R: Read> {
    #[cfg(feature = "archive-tar")]
    TarEntry(tar::TarEntryRaw<'a, R>),
    #[cfg(feature = "archive-zip")]
    ZipEntry(zip::ZipEntryRaw<'a>),
}

impl<'a, R: Read> ArchiveEntry<'a, R> {
    pub fn path(&self) -> Result<PathBuf> {
        Ok(match self {
            #[cfg(feature = "archive-tar")]
            Self::TarEntry(entry) => entry.path()?.into(),
            #[cfg(feature = "archive-zip")]
            Self::ZipEntry(entry) => entry.enclosed_name().ok_or_else(|| anyhow!("Failed to parse zip entry name. It contains illegal parts {}", entry.name()))?
        })
    }
    
    pub fn size(&self) -> u64 {
        match self {
            #[cfg(feature = "archive-tar")]
            Self::TarEntry(entry) => entry.size(),
            #[cfg(feature = "archive-zip")]
            Self::ZipEntry(entry) => entry.size(),
        }
    }
    
    pub fn modified(&self) -> u64 {
        match self {
            #[cfg(feature = "archive-tar")]
            Self::TarEntry(entry) => entry.header().mtime().unwrap_or(0),
            #[cfg(feature = "archive-zip")]
            Self::ZipEntry(entry) => entry.last_modified().map(|datetime| {
                    let ymd = NaiveDate::from_ymd_opt(datetime.year() as i32, datetime.month() as u32, datetime.day() as u32);
                    let hms = NaiveTime::from_hms_opt(datetime.hour() as u32, datetime.minute() as u32, datetime.second() as u32);
                
                    if ymd.is_none() || hms.is_none() {
                        NaiveDateTime::UNIX_EPOCH
                    } else {
                        NaiveDateTime::new(ymd.unwrap(), hms.unwrap())
                    }
                })
                .map(|naive_datetime| naive_datetime.signed_duration_since(NaiveDateTime::UNIX_EPOCH).num_seconds() as u64).unwrap_or(0)
        }
    }
    
    pub fn stream(&mut self) -> Box<&mut dyn Read> {
        match self {
            #[cfg(feature = "archive-tar")]
            Self::TarEntry(entry) => Box::new(entry),
            #[cfg(feature = "archive-zip")]
            Self::ZipEntry(entry) => Box::new(entry),
        }
    }
}

impl<'a, R: Read> Read for ArchiveEntry<'a, R> {
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
        match self { 
            #[cfg(feature = "archive-tar")]
            Self::TarEntry(entry) => entry.read(buf),
            #[cfg(feature = "archive-zip")]
            Self::ZipEntry(entry) => entry.read(buf),
        }
    }
}

impl<'a, R: Read> Debug for ArchiveEntry<'a, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArchiveEntry {{ path: {:?}, size: {}, modified: {} }}", self.path(), self.size(), self.modified())
    }
}

#[cfg(feature = "archive-tar")]
pub mod tar;
#[cfg(feature = "archive-zip")]
pub mod zip;

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

