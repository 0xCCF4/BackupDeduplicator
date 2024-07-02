use crate::archive::ArchiveEntry;
use anyhow::{anyhow, Result};
use std::io::Read;
use std::pin::Pin;
use zip;

/// A Zip archive.
pub struct ZipArchive<R: Read> {
    reader: Pin<Box<R>>,
}

impl<R: Read> ZipArchive<R> {
    /// Create a new Zip archive from a reader.
    /// Does not perform any checking on the input. The input is assumed to be a valid Zip archive.
    /// 
    /// # Arguments
    /// * `input` - The input reader.
    /// 
    /// # Returns
    /// The Zip archive.
    /// 
    /// # Errors
    /// Never.
    pub fn new(input: R) -> Result<ZipArchive<R>> {
        Ok(ZipArchive {
            reader: Box::pin(input),
        })
    }

    /// Get the entries of the Zip archive.
    /// 
    /// # Returns
    /// The entries of the Zip archive.
    /// 
    /// # Errors
    /// Never
    pub fn entries(&mut self) -> Result<ZipArchiveIterator<R>> {
        Ok(ZipArchiveIterator::new(&mut self.reader))
    }
}

/// An iterator over the entries of a Zip archive.
pub struct ZipArchiveIterator<'a, R: Read> {
    reader: &'a mut Pin<Box<R>>,
}

impl<'a, R: Read> ZipArchiveIterator<'a, R> {
    /// Create a new Zip archive iterator from a reader.
    /// 
    /// # Arguments
    /// * `reader` - The reader.
    /// 
    /// # Returns
    /// The Zip archive iterator.
    pub fn new(reader: &'a mut Pin<Box<R>>) -> ZipArchiveIterator<'a, R> {
        ZipArchiveIterator { reader }
    }
}

impl<'a, R: Read> Iterator for ZipArchiveIterator<'a, R> {
    type Item = Result<ArchiveEntry<'a, R>>;
    fn next(&mut self) -> Option<Self::Item> {
        // should be safe, since:
        //   - &mut ZipArchiveIterator requires &mut Pin<Box<R>> to live
        //   - &mut Pin<Box<R>> requires &mut R to live
        //   - &mut R requires ZipArchive to live
        //   - ZipArchive requires R to live
        // this function cannot be called again as long the returned valued is alive
        // the returned value must be dropped first
        let stream_ref: &mut R = unsafe { std::mem::transmute(self.reader.as_mut()) };

        let file = zip::read::read_zipfile_from_stream(stream_ref);
        let file = match file {
            Ok(file) => file,
            Err(err) => return Some(Err(anyhow!("Failed to read Zip entry: {}", err))),
        };

        let file = file.use_untrusted_value();
        // This is safe, since we are not extracting the file and our attacker model does not
        // cover an attacker that can influence the content of the given zip file

        let file = match file {
            Some(file) => file,
            None => return None,
        };

        Some(Ok(ArchiveEntry::ZipEntry(file)))
    }
}

pub use zip::read::ZipFile as ZipEntryRaw;
