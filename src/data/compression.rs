use std::io::Read;
use anyhow::{Result};
use serde::{Deserialize, Serialize};
use crate::utils;

/// Compression type
/// 
/// # Fields
/// * `Gz` - Gzip compression. Enabled by the `compress-flate2` feature.
/// * `Xz` - Xz compression. Enabled by the `compress-xz` feature.
/// * `None` - No compression.
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq)]
pub enum CompressionType {
    #[cfg(feature = "compress-flate2")]
    Gz,
    #[cfg(feature = "compress-xz")]
    Xz,
    Null,
}

impl CompressionType {
    /// Create a decompressor for the compression type.
    /// 
    /// # Arguments
    /// * `input` - The input to decompress.
    /// 
    /// # Returns
    /// A decompressed stream.
    pub fn open<R: Read + 'static>(&self, input: R) -> Box<dyn Read> {
        match self {
            #[cfg(feature = "compress-flate2")]
            CompressionType::Gz => Box::new(flate2::read::GzDecoder::new(input)),
            #[cfg(feature = "compress-xz")]
            CompressionType::Xz => Box::new(xz2::read::XzDecoder::new(input)),
            CompressionType::Null => Box::new(input),
        }
    }
    
    /// Get the compression type from the file extension.
    /// 
    /// # Arguments
    /// * `extension` - The file extension.
    /// 
    /// # Returns
    /// The compression type.
    pub fn from_extension(extension: &str) -> CompressionType {
        match extension {
            #[cfg(feature = "compress-flate2")]
            "gz" => CompressionType::Gz,
            #[cfg(feature = "compress-xz")]
            "xz" => CompressionType::Xz,
            _ => CompressionType::Null,
        }
    }
    
    /// Get the maximum amount of bytes to peek from the stream to determine the compression type.
    /// 
    /// # Returns
    /// The maximum amount of bytes to peek needed to determine the compression type.
    pub const fn max_stream_peek_count() -> usize {
        const MAX_BYTES_FLATE: usize = match cfg!(feature = "compress-flate2") {
            true => 2,
            false => 0
        };
        const MAX_BYTES_XZ: usize = match cfg!(feature = "compress-xz") {
            true => 6,
            false => 0
        };
        const MAX_BYTES: usize = utils::max(MAX_BYTES_FLATE, MAX_BYTES_XZ);
        
        MAX_BYTES
    }

    /// Get the compression type from the stream.
    /// 
    /// # Arguments
    /// * `stream` - The stream to read from.
    /// 
    /// # Returns
    /// The compression type.
    /// 
    /// # Errors
    /// If the stream could not be read.
    /// 
    /// # Notes
    /// This function reads the first few bytes of the stream to determine the compression type.
    /// The default use case would probably be using [BufferCopyStreamReader] to proxy
    /// the actual stream and then pass the original stream to the decompressor.
    ///
    /// # Example
    /// ```
    /// # use std::fs::File;
    /// # use backup_deduplicator::archive::ArchiveType;
    /// # use backup_deduplicator::compression::CompressionType;
    /// # use backup_deduplicator::copy_stream::BufferCopyStreamReader;
    ///
    /// let file = File::open("tests/res/archive_example.tar.gz").unwrap();
    /// let stream = BufferCopyStreamReader::with_capacity(file, CompressionType::max_stream_peek_count());
    ///
    /// let compression_type = CompressionType::from_stream(stream.child()).unwrap();
    /// assert_eq!(compression_type, CompressionType::Gz);
    ///
    /// let decompressed = compression_type.open(stream.try_into_inner().unwrap());
    ///
    /// let mut archive = ArchiveType::Tar.open(decompressed).unwrap();
    /// let entry = archive.next().unwrap();
    /// assert_eq!(entry.unwrap().path.to_str().unwrap(), "tar_root/");
    /// ```
    pub fn from_stream<R: Read>(stream: R) -> Result<CompressionType> {
        const MAX_BYTES: usize = CompressionType::max_stream_peek_count();
        
        if MAX_BYTES == 0 {
            return Ok(CompressionType::Null);
        }
        
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
        
        #[cfg(feature = "compress-flate2")]
        if buffer[0] == 0x1F && buffer[1] == 0x8B {
            return Ok(CompressionType::Gz);
        }
        
        #[cfg(feature = "compress-xz")]
        if buffer[0] == 0xFD && buffer[1] == 0x37 && buffer[2] == 0x7A && buffer[3] == 0x58 && buffer[4] == 0x5A && buffer[5] == 0x00 {
            return Ok(CompressionType::Xz);
        }
        
        Ok(CompressionType::Null)
    }
}
