use std::io::Read;
use serde::{Deserialize, Serialize};

/// Compression type
/// 
/// # Fields
/// * `Gz` - Gzip compression. Enabled by the `compress-flate2` feature.
/// * `Xz` - Xz compression. Enabled by the `compress-xz` feature.
/// * `None` - No compression.
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
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
    
    pub fn from_extension(extension: &str) -> Option<CompressionType> {
        match extension {
            #[cfg(feature = "compress-flate2")]
            "gz" => Some(CompressionType::Gz),
            #[cfg(feature = "compress-xz")]
            "xz" => Some(CompressionType::Xz),
            _ => None,
        }
    }
}
