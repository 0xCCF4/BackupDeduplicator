use std::cell::RefCell;
use std::io::Read;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use log::error;
use crate::archive::{ArchiveEntry, ArchiveIterator};
use anyhow::Result;


/// An iterator over a tar archive.
/// 
/// # Example
/// ```
/// use std::fs::File;
/// use std::io::Read;
/// use backup_deduplicator::archive::tar::TarArchiveIterator;
/// use backup_deduplicator::compression::CompressionType;
///
/// let file = std::fs::File::open("tests/res/archive_example.tar.gz").expect("Test resource not found.");
/// let decompfile = CompressionType::Gz {}.open(file);
/// let archive = TarArchiveIterator::new(decompfile).unwrap();
/// for mut entry in archive {
///     println!("{:?}", entry.unwrap().path);
/// }
/// ``` 
#[allow(dead_code)]
pub struct TarArchiveIterator<'a, R: Read> {
    // Self-referential struct, therefore drop must occur in the order entries -> archive -> input.
    // DO NOT CHANGE THE ORDER OF THESE FIELDS.
    entries: Pin<Box<tar::Entries<'a, &'a mut R>>>,
    archive: Pin<Box<tar::Archive<&'a mut R>>>,
    input: Pin<Box<R>>,
    current_entry_raw: Option<Pin<Box<tar::Entry<'a, &'a mut R>>>>,
    current_entry: Option<ArchiveEntry>,

    // Used to ensure that the previous entry is consumed before the next one is returned.
    // Since we return a stream to the entry, we need to ensure that the previous stream is consumed before
    // running destructors on the previous entry.
    entry_active: Rc<RefCell<bool>>,

    _phantom_data: std::marker::PhantomData<&'a R>,
}

impl<'a, R: Read + 'a> TarArchiveIterator<'a, R> {
    /// Create a new tar archive iterator.
    /// 
    /// # Arguments
    /// * `input` - The input stream.
    /// 
    /// # Returns
    /// The tar archive iterator.
    /// 
    /// # Errors
    /// If the tar archive cannot be read.
    pub fn new(input: R) -> anyhow::Result<TarArchiveIterator<'a, R>> {
        let mut input = Box::pin(input);
        let input_ref: &'a mut R = unsafe { std::mem::transmute(input.as_mut()) };

        let archive = tar::Archive::new(input_ref);
        let mut archive = Box::pin(archive);
        let archive_ref: &'a mut tar::Archive<&'a mut R> = unsafe { std::mem::transmute(archive.as_mut()) };

        let entries = archive_ref.entries()?;
        let entries = Box::pin(entries);
        // let mut entries_ref: &'a mut tar::Entries<'a, &'a mut R> = unsafe { std::mem::transmute(&mut entries) };
        
        Ok(TarArchiveIterator {
            input,
            archive,
            entries,

            entry_active: Rc::new(RefCell::new(false)),
            current_entry: None,
            current_entry_raw: None,
            _phantom_data: std::marker::PhantomData,
        })
    }
}

impl<'a, R: Read> Drop for TarArchiveIterator<'a, R> {
    fn drop(&mut self) {
        let prev = self.entry_active.replace_with(|_| false);
        if prev {
            panic!("Memory state invalid. Previous entry was not consumed.");
        }
        drop(self.current_entry.take());
    }
}

struct TarEntryStream<'a, R: Read> {
    iterator: Rc<RefCell<bool>>,
    entry: &'a mut tar::Entry<'a, &'a mut R>,
}

impl<'a, R: Read> Read for TarEntryStream<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.entry.read(buf)
    }
}

impl<'a, R: Read> Drop for TarEntryStream<'a, R> {
    fn drop(&mut self) {
        let prev = self.iterator.replace_with(|_| false);
        if !prev {
            panic!("Illegal state: Previous entry was already consumed.");
        }
    }
}

impl<'a, R: Read> ArchiveIterator<'a> for TarArchiveIterator<'a, R> {
    
    /// Get the next entry in the tar archive.
    /// 
    /// # Returns
    /// The next entry in the tar archive. If any.
    fn next(&mut self) -> Option<Result<()>> {
        let next = self.entries.next();

        match next {
            None => return None,
            Some(Err(e)) => {
                error!("Failed to read entry from tar archive: {}", e);
                return None;
            },
            Some(Ok(entry)) => {
                let path = entry.path().map_err(
                    |e| anyhow::anyhow!("Failed to read path from tar archive: {}", e)
                );
                let path = match path {
                    Ok(path) => path,
                    Err(e) => {
                        return Some(Err(e));
                    }
                };
                let path: PathBuf = path.into();
                let size = entry.size();
                let modified = entry.header().mtime().unwrap_or(0);

                let mut entry = Box::pin(entry);
                let entry_ref: &'a mut tar::Entry<'a, &'a mut R> = unsafe { std::mem::transmute(entry.as_mut()) };
                
                self.current_entry_raw = Some(entry);

                let archive_entry = ArchiveEntry {
                    path,
                    size,
                    modified,
                    stream: Box::new(TarEntryStream {
                        iterator: self.entry_active.clone(),
                        entry: entry_ref,
                    }) as Box<dyn Read>,
                };

                drop(self.current_entry.take());
                let prev = self.entry_active.replace_with(|_| true);
                if prev {
                    panic!("Previous entry was not consumed.");
                }

                self.current_entry = Some(archive_entry);

                Some(Ok(()))
            }
        }
    }

    fn current_entry(&mut self) -> Option<&mut ArchiveEntry> {
        match self.current_entry {
            Some(ref mut entry) => Some(entry),
            None => None,
        }
    }
}
