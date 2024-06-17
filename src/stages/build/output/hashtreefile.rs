use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::ops::DerefMut;
use std::sync::Arc;

use anyhow::Result;
use log::{info, trace, warn};
use serde::{Deserialize, Serialize};

pub use HashTreeFileEntryTypeV1 as HashTreeFileEntryType;
pub use HashTreeFileEntryV1 as HashTreeFileEntry;
pub type HashTreeFileEntryRef<'a> = HashTreeFileEntryV1Ref<'a>;

use crate::hash::{GeneralHash, GeneralHashType};
use crate::path::FilePath;
use crate::utils;

/// HashTreeFile file version. In further versions, the file format may change.
/// Currently only one file version exist.
///
/// # Fields
/// * `V1` - Version 1 of the file format.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HashTreeFileVersion {
    V1,
}

/// HashTreeFile file header. First line of a hash tree file.
///
/// # Fields
/// * `version` - The version of the file.
/// * `hash_type` - The hash type used to hash the files.
/// * `creation_date` - The creation date of the file in unix time
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HashTreeFileHeader {
    pub version: HashTreeFileVersion,
    pub hash_type: GeneralHashType,
    pub creation_date: u64,
}

/// HashTreeFile entry type. Describes the type of file.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
pub enum HashTreeFileEntryTypeV1 {
    File,
    Directory,
    Symlink,
    Other,
}

/// HashTreeFile entry. Describes an analyzed file.
///
/// # Fields
/// * `file_type` - The type of the file.
/// * `modified` - The last modified date of the file in unix time.
/// * `size` - The size of the file in bytes for files, number of children for folders.
/// * `hash` - The hash of the file content.
/// * `path` - The path of the file.
/// * `children` - The children of the file. Only for directories.
/// * `archive_children` - The children of this file if it is an archive.
///
/// # See also
/// * [HashTreeFileEntryV1Ref] which is a reference version of this struct.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct HashTreeFileEntryV1 {
    pub file_type: HashTreeFileEntryTypeV1,
    pub modified: u64,
    pub size: u64,
    pub hash: GeneralHash,
    pub path: FilePath,
    pub children: Vec<GeneralHash>,
    pub archive_children: Vec<HashTreeFileEntryV1>,
}

/// HashTreeFile entry reference. Describes an analyzed file.
/// This is a reference version of the [HashTreeFileEntryV1] struct.
/// 
/// # Fields
/// * `file_type` - The type of the file.
/// * `modified` - The last modified date of the file in unix time.
/// * `size` - The size of the file in bytes for files, number of children for folders.
/// * `hash` - The hash of the file content.
/// * `path` - The path of the file.
/// * `children` - The children of the file. Only for directories.
/// * `archive_children` - The children of this file if it is an archive.
/// 
/// # See also
/// * [HashTreeFileEntryV1] which is the owned version of this struct.
#[derive(Debug, Serialize)]
pub struct HashTreeFileEntryV1Ref<'a> {
    pub file_type: &'a HashTreeFileEntryTypeV1,
    pub modified: &'a u64,
    pub size: &'a u64,
    pub hash: &'a GeneralHash,
    pub path: &'a FilePath,
    pub children: Vec<&'a GeneralHash>,
    pub archive_children: Vec<HashTreeFileEntryV1>,
}

/// Interface to access and manage a hash tree file.
/// 
/// # Fields
/// * `header` - The header of the file.
/// * `file_by_hash` - A map of files by their hash.
/// * `file_by_path` - A map of files by their path.
/// * `all_entries` - A list of all entries.
pub struct HashTreeFile<'a, W, R> where W: Write, R: BufRead {
    pub header: HashTreeFileHeader,
    pub file_by_hash: HashMap<GeneralHash, Vec<Arc<HashTreeFileEntry>>>,
    pub file_by_path: HashMap<FilePath, Arc<HashTreeFileEntry>>,
    pub all_entries: Vec<Arc<HashTreeFileEntry>>,
    
    enable_file_by_hash: bool,
    enable_file_by_path: bool,
    enable_all_entry_list: bool,
    
    writer: RefCell<&'a mut W>,
    written_bytes: RefCell<usize>,
    reader: RefCell<&'a mut R>,
}

impl<'a, W: Write, R: BufRead> HashTreeFile<'a, W, R> {
    /// Create a new hash tree file.
    /// 
    /// If not writing a new header hash_type can be set to GeneralHashType::NULL. 
    /// 
    /// # Arguments
    /// * `writer` - The writer to write the file.
    /// * `reader` - The reader to read the file.
    /// * `hash_type` - The hash type used to hash the files.
    /// * `enable_file_by_hash` - Whether to enable the file by hash - hash map.
    /// * `enable_file_by_path` - Whether to enable the file by path - hash map.
    /// * `enable_all_entry_list` - Whether to enable the all entries list.
    /// 
    /// # Returns
    /// The created hash tree file interface.
    pub fn new(writer: &'a mut W, reader: &'a mut R, hash_type: GeneralHashType, enable_file_by_hash: bool, enable_file_by_path: bool, enable_all_entry_list: bool) -> Self {
        let time = utils::get_time();
        HashTreeFile {
            header: HashTreeFileHeader {
                version: HashTreeFileVersion::V1,
                hash_type,
                creation_date: time,
            },
            file_by_hash: HashMap::new(),
            file_by_path: HashMap::new(),
            all_entries: Vec::new(),
            enable_file_by_hash,
            enable_file_by_path,
            enable_all_entry_list,
            writer: RefCell::new(writer),
            reader: RefCell::new(reader),
            written_bytes: RefCell::new(0),
        }
    }
    
    /// Save the header to the file
    /// 
    /// # Error
    /// If writing to the file errors
    pub fn save_header(&self) -> Result<()> {
        let header_str = serde_json::to_string(&self.header)?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write(header_str.as_bytes())?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write(b"\n")?;
        
        Ok(())
    }
    
    /// Load a file header from the file
    /// 
    /// # Error
    /// If reading from the file errors
    pub fn load_header(&mut self) -> Result<()> {
        let mut header_str = String::new();
        self.reader.borrow_mut().deref_mut().read_line(&mut header_str)?;
        
        let header: HashTreeFileHeader = serde_json::from_str(header_str.as_str())?;
        self.header = header;
        
        Ok(())
    }

    /// Load a file entry from the file
    /// 
    /// # Error
    /// If reading from the file errors
    pub fn load_entry_no_filter(&mut self) -> Result<Option<Arc<HashTreeFileEntry>>> {
        self.load_entry(|_| true)
    }
    
    /// Load a file entry from the file
    /// 
    /// # Arguments
    /// * `filter` - A filter function to filter the entries. If the function returns false the entry is ignored.
    /// 
    /// # Returns
    /// The loaded entry or None if the end of the file is reached.
    /// 
    /// # Error
    /// If reading from the file errors
    pub fn load_entry<F: Fn(&HashTreeFileEntry) -> bool>(&mut self, filter: F) -> Result<Option<Arc<HashTreeFileEntry>>> {
        loop {
            let mut entry_str = String::new();
            let count = self.reader.borrow_mut().deref_mut().read_line(&mut entry_str)?;

            if count == 0 {
                return Ok(None);
            }
            
            if count == 1 {
                continue;
            }

            let entry: HashTreeFileEntry = serde_json::from_str(entry_str.as_str())?;

            if entry.hash.hash_type() != self.header.hash_type && !(entry.file_type == HashTreeFileEntryType::Other && entry.hash.hash_type() == GeneralHashType::NULL) {
                warn!("Hash type mismatch ignoring entry: {:?}", entry.path);
                continue;
            }
            
            if !filter(&entry) {
                trace!("Entry filtered: {:?}", entry.path);
                continue;
            }

            let shared_entry = Arc::new(entry);

            if self.enable_file_by_hash {
                self.file_by_hash.entry(shared_entry.hash.clone()).or_insert_with(Vec::new).push(Arc::clone(&shared_entry));
            }

            if self.enable_file_by_path {
                match self.file_by_path.insert(shared_entry.path.clone(), Arc::clone(&shared_entry)) {
                    None => {}
                    Some(old) => {
                        // this happens if analysis was canceled and continued
                        // and an already analysed file changed
                        info!("Duplicate entry for path: {:?}", &old.path);
                        if self.enable_all_entry_list {
                            self.all_entries.retain(|x| x != &old);
                        }
                    }
                }
            }

            if self.enable_all_entry_list {
                self.all_entries.push(Arc::clone(&shared_entry));
            }

            return Ok(Some(shared_entry))
        }
    }
    
    /// Load all entries from the file. Till the end of the file is reached.
    /// 
    /// # Arguments
    /// * `filter` - A filter function to filter the entries. If the function returns false the entry is ignored.
    /// 
    /// # Error
    /// If reading from the file errors
    pub fn load_all_entries<F: Fn(&HashTreeFileEntry) -> bool>(&mut self, filter: F) -> Result<()> {
        while let Some(_) = self.load_entry(&filter)? {}
        
        Ok(())
    }

    /// Load all entries from the file. Till the end of the file is reached.
    /// 
    /// # Error
    /// If reading from the file errors
    pub fn load_all_entries_no_filter(&mut self) -> Result<()> {
        self.load_all_entries(|_| true)
    }

    /// Write an entry to the file
    /// 
    /// # Arguments
    /// * `result` - The entry to write.
    /// 
    /// # Error
    /// If writing to the file errors
    pub fn write_entry(&self, result: &HashTreeFileEntry) -> Result<()> {
        let string = serde_json::to_string(result)?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write(string.as_bytes())?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write("\n".as_bytes())?;
        self.writer.borrow_mut().deref_mut().flush()?;
        Ok(())
    }

    /// Write an entry reference to the file
    /// 
    /// # Arguments
    /// * `result` - The entry reference to write.
    /// 
    /// # Error
    /// If writing to the file errors
    pub fn write_entry_ref(&self, result: &HashTreeFileEntryRef) -> Result<()> {
        let string = serde_json::to_string(result)?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write(string.as_bytes())?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write("\n".as_bytes())?;
        self.writer.borrow_mut().deref_mut().flush()?;
        Ok(())
    }
    
    /// Empty the file by hash - hash map.
    /// Frees/Shrinks the memory used.
    pub fn empty_file_by_hash(&mut self) {
        self.file_by_hash.clear();
        self.file_by_hash.shrink_to_fit();
    }
    
    /// Empty the file by path - hash map.
    /// Frees/Shrinks the memory used.
    pub fn empty_file_by_path(&mut self) {
        self.file_by_path.clear();
        self.file_by_path.shrink_to_fit();
    }

    /// Empty the all entries list.
    /// Frees/Shrinks the memory used.
    pub fn empty_entry_list(&mut self) {
        self.all_entries.clear();
        self.all_entries.shrink_to_fit();
    }

    /// Get the written bytes count.
    /// 
    /// # Returns
    /// The written bytes count.
    pub fn get_written_bytes(&self) -> usize {
        *self.written_bytes.borrow()
    }
    
    /// Flush the writer.
    /// 
    /// # Error
    /// If flushing the writer errors
    pub fn flush(&self) -> std::io::Result<()> {
        self.writer.borrow_mut().deref_mut().flush()
    }
}
