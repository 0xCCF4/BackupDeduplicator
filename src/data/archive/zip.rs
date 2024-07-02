use std::io::{Read};
use std::pin::Pin;
use anyhow::{anyhow, Result};
use zip;
use crate::archive::ArchiveEntry;

pub struct ZipArchive<R: Read> {
    reader: Pin<Box<R>>,
}

impl<R: Read> ZipArchive<R> {
    pub fn new(input: R) -> Result<ZipArchive<R>> {
        Ok(ZipArchive {
            reader: Box::pin(input),
        })
    }

    pub fn entries(&mut self) -> Result<ZipArchiveIterator<R>> {
        Ok(ZipArchiveIterator::new(&mut self.reader))
    }
}

pub struct ZipArchiveIterator<'a, R: Read> {
    reader: &'a mut Pin<Box<R>>,
}

impl<'a, R: Read> ZipArchiveIterator<'a, R> {
    pub fn new(reader: &'a mut Pin<Box<R>>) -> ZipArchiveIterator<'a, R> {
        ZipArchiveIterator {
            reader
        }
    }
}

impl<'a, R: Read> Iterator for ZipArchiveIterator<'a, R> {
    type Item = Result<ArchiveEntry<'a, R>>;
    fn next(&mut self) -> Option<Self::Item> {
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