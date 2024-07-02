use std::io::Read;
use anyhow::{anyhow, Result};
use crate::archive::ArchiveEntry;

pub struct TarArchive<R: Read> {
    archive: tar::Archive<R>,
}

impl<'a, R: Read> TarArchive<R> {
    pub fn new(input: R) -> Result<TarArchive<R>> {
        Ok(TarArchive {
            archive: tar::Archive::new(input),
        })
    }
    
    pub fn entries(&'a mut self) -> Result<TarArchiveIterator<'a, R>> {
        Ok(TarArchiveIterator::new(self.archive.entries()?))
    }
}

pub struct TarArchiveIterator<'a, R: 'a + Read> {
    entries: tar::Entries<'a, R>,
}

impl<'a, R: Read> TarArchiveIterator<'a, R> {
    pub fn new(entries: tar::Entries<'a, R>) -> TarArchiveIterator<'a, R> {
        TarArchiveIterator {
            entries,
        }
    }
}

impl<'a, R: Read> Iterator for TarArchiveIterator<'a, R> {
    type Item = Result<ArchiveEntry<'a, R>>;
    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(
            | result | result
                .map(ArchiveEntry::TarEntry)
                .map_err(|err| anyhow!("Failed to read tar entry: {}", err)))
    }
}

pub use tar::Entry as TarEntryRaw;