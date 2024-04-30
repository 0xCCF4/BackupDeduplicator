use std::io::Read;
use std::path::PathBuf;
use log::error;
use crate::archive::ArchiveEntry;

pub struct TarArchiveIterator<'a, R: Read + 'a + 'static> {
    input: R, // fully ownership of the input
    archive: Option<tar::Archive<&'a mut R>>,
    entries: Option<tar::Entries<'a, &'a mut R>>,
    current_entry: Option<tar::Entry<'a, &'a mut R>>,
}

impl<'a, R: Read> TarArchiveIterator<'a, R> {
    pub fn new(input: R) -> anyhow::Result<TarArchiveIterator<'a, R>> {
        let mut iterator = TarArchiveIterator {
            input,
            archive: None,
            entries: None,
            current_entry: None,
        };
        
        unsafe {
            let pointer = &mut iterator.input as *mut R;
            let input = &mut *pointer;
            let archive = tar::Archive::new(input);
            iterator.archive = Some(archive);
        }
        
        unsafe {
            let pointer = &mut iterator.archive as *mut Option<tar::Archive<&'a mut R>>;
            let archive = &mut *pointer;
            let entries = archive.as_mut().unwrap().entries()?;
            iterator.entries = Some(entries);
        }
        
        Ok(iterator)
    }
    
    pub fn next(&'a mut self) -> Option<ArchiveEntry> {
        let entry = self.entries.as_mut()?.next();
        match entry {
            None => { 
                self.current_entry = None;
                None
            },
            Some(entry) => {
                match entry {
                    Ok(entry) => {
                        let path;

                        match entry.path() {
                            Ok(p) => {
                                path = p;
                            },
                            Err(_) => {
                                error!("Failed to get path from tar entry.");
                                self.current_entry = None;
                                return None;
                            }
                        }

                        let path: PathBuf = path.into();
                        let size = entry.size();

                        self.current_entry = Some(entry);

                        let archive_entry = ArchiveEntry {
                            path,
                            size,
                        };

                        Some(archive_entry)
                    },
                    Err(_) => {
                        error!("Failed to read entry from tar archive.");
                        self.current_entry = None;
                        None
                    }
                }
            }
        }
    }
    
    pub fn drop(&mut self) {
        // If current entry is some, drop it.
        drop(self.current_entry.take());
        drop(self.entries.take());
        drop(self.archive.take());
    }
    
    pub fn release(self) -> R {
        self.input
    }
}

impl<'a, R: Read> Read for TarArchiveIterator<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.current_entry.as_mut() {
            None => Err(std::io::Error::new(std::io::ErrorKind::Other, "No current entry.")),
            Some(entry) => entry.read(buf),
        }
    }
}
