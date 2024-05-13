use std::cell::RefCell;
use std::io::Read;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use log::error;
use crate::archive::ArchiveEntry;
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
pub struct TarArchiveIterator<R: Read + 'static> {
    // Self-referential struct, therefore drop must occur in the order entries -> archive -> input.
    // DO NOT CHANGE THE ORDER OF THESE FIELDS.
    entries: Pin<Box<tar::Entries<'static, &'static mut R>>>,
    archive: Pin<Box<tar::Archive<&'static mut R>>>,
    input: Pin<Box<R>>,

    // Used to ensure that the previous entry is consumed before the next one is returned.
    // Since we return a stream to the entry, we need to ensure that the previous stream is consumed before
    // running destructors on the previous entry.
    entry_active: Rc<RefCell<bool>>,
}

impl<R: Read> TarArchiveIterator<R> {
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
    pub fn new(input: R) -> anyhow::Result<TarArchiveIterator<R>> {
        let mut input = Box::pin(input);
        let input_ref: &'static mut R = unsafe { std::mem::transmute(input.as_mut()) };

        let archive = tar::Archive::new(input_ref);
        let mut archive = Box::pin(archive);
        let archive_ref: &'static mut tar::Archive<&'static mut R> = unsafe { std::mem::transmute(archive.as_mut()) };

        let entries = archive_ref.entries()?;
        let entries = Box::pin(entries);
        // let mut entries_ref: &'static mut tar::Entries<'static, &'static mut R> = unsafe { std::mem::transmute(&mut entries) };
        
        Ok(TarArchiveIterator {
            input,
            archive,
            entries,

            entry_active: Rc::new(RefCell::new(false)),
        })
    }
}

impl<R: Read> Drop for TarArchiveIterator<R> {
    fn drop(&mut self) {
        let prev = self.entry_active.replace_with(|_| false);
        if prev {
            panic!("Memory state invalid. Previous entry was not consumed.");
        }
    }
}

struct TarEntryStream<R: Read + 'static> {
    iterator: Rc<RefCell<bool>>,
    entry: tar::Entry<'static, &'static mut R>,
}

impl<R: Read> Read for TarEntryStream<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.entry.read(buf)
    }
}

impl<R: Read> Drop for TarEntryStream<R> {
    fn drop(&mut self) {
        let prev = self.iterator.replace_with(|_| false);
        if !prev {
            panic!("Illegal state: Previous entry was already consumed.");
        }
    }
}

impl<R: Read> Iterator for TarArchiveIterator<R> {
    type Item = Result<ArchiveEntry>;
    
    /// Get the next entry in the tar archive.
    /// 
    /// # Returns
    /// The next entry in the tar archive. If any.

    fn next(&mut self) -> Option<Result<ArchiveEntry>> {
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

                let archive_entry = ArchiveEntry {
                    path,
                    size,
                    modified,
                    stream: Box::new(TarEntryStream {
                        iterator: Rc::clone(&self.entry_active),
                        entry,
                    }),
                };

                let prev = self.entry_active.replace_with(|_| true);
                if prev {
                    panic!("Previous entry was not consumed.");
                }

                Some(Ok(archive_entry))
            }
        }
    }
}
