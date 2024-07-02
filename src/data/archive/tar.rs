use crate::archive::ArchiveEntry;
use anyhow::{anyhow, Result};
use std::io::Read;

/// A Tar archive.
pub struct TarArchive<R: Read> {
    archive: tar::Archive<R>,
}

impl<'a, R: Read> TarArchive<R> {
    /// Create a new Tar archive from a reader.
    ///
    /// # Arguments
    /// * `input` - The input reader.
    ///
    /// # Returns
    /// The Tar archive.
    ///
    /// # Errors
    /// If the archive is invalid or cannot be read.
    pub fn new(input: R) -> Result<TarArchive<R>> {
        Ok(TarArchive {
            archive: tar::Archive::new(input),
        })
    }

    /// Get the entries of the Tar archive.
    ///
    /// # Returns
    /// An iterator over the entries of the Tar archive.
    ///
    /// # Errors
    /// If the entries cannot be read.
    pub fn entries(&'a mut self) -> Result<TarArchiveIterator<'a, R>> {
        Ok(TarArchiveIterator::new(self.archive.entries()?))
    }
}

/// An iterator over the entries of a Tar archive.
pub struct TarArchiveIterator<'a, R: 'a + Read> {
    entries: tar::Entries<'a, R>,
}

impl<'a, R: Read> TarArchiveIterator<'a, R> {
    /// Create a new Tar archive iterator from a reader.
    ///
    /// # Arguments
    /// * `entries` - The entries.
    ///
    /// # Returns
    /// The Tar archive iterator.
    pub fn new(entries: tar::Entries<'a, R>) -> TarArchiveIterator<'a, R> {
        TarArchiveIterator { entries }
    }
}

impl<'a, R: Read> Iterator for TarArchiveIterator<'a, R> {
    type Item = Result<ArchiveEntry<'a, R>>;
    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(|result| {
            result
                .map(ArchiveEntry::TarEntry)
                .map_err(|err| anyhow!("Failed to read tar entry: {}", err))
        })
    }
}

pub use tar::Entry as TarEntryRaw;
