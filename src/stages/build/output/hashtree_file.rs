use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::ops::DerefMut;
use std::sync::Arc;

use anyhow::Result;
use log::{info, trace, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SaveFileVersion {
    V1,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SaveFileHeaders {
    pub version: SaveFileVersion,
    pub hash_type: GeneralHashType,
    pub creation_date: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
pub enum SaveFileEntryTypeV1 {
    File,
    Directory,
    Symlink,
    Other,
}
pub use SaveFileEntryTypeV1 as SaveFileEntryType;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SaveFileEntryV1 {
    pub file_type: SaveFileEntryTypeV1,
    pub modified: u64,
    pub size: u64,
    pub hash: GeneralHash,
    pub path: FilePath,
    pub children: Vec<GeneralHash>,
}
pub use SaveFileEntryV1 as SaveFileEntry;
use crate::hash::{GeneralHash, GeneralHashType};
use crate::path::FilePath;
use crate::utils;

#[derive(Debug, Serialize)]
pub struct SaveFileEntryV1Ref<'a> {
    pub file_type: &'a SaveFileEntryTypeV1,
    pub modified: &'a u64,
    pub size: &'a u64,
    pub hash: &'a GeneralHash,
    pub path: &'a FilePath,
    pub children: Vec<&'a GeneralHash>,
}
pub type SaveFileEntryRef<'a> = SaveFileEntryV1Ref<'a>;

pub struct SaveFile<'a, W, R> where W: Write, R: BufRead {
    pub header: SaveFileHeaders,
    pub file_by_hash: HashMap<GeneralHash, Vec<Arc<SaveFileEntry>>>,
    pub file_by_path: HashMap<FilePath, Arc<SaveFileEntry>>,
    pub all_entries: Vec<Arc<SaveFileEntry>>,
    
    enable_file_by_hash: bool,
    enable_file_by_path: bool,
    enable_all_entry_list: bool,
    
    writer: RefCell<&'a mut W>,
    written_bytes: RefCell<usize>,
    reader: RefCell<&'a mut R>,
}

impl<'a, W: Write, R: BufRead> SaveFile<'a, W, R> {
    pub fn new(writer: &'a mut W, reader: &'a mut R, enable_file_by_hash: bool, enable_file_by_path: bool, enable_all_entry_list: bool) -> Self {
        let time = utils::get_time();
        SaveFile {
            header: SaveFileHeaders {
                version: SaveFileVersion::V1,
                hash_type: GeneralHashType::SHA256,
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
    
    pub fn save_header(&self) -> Result<()> {
        let header_str = serde_json::to_string(&self.header)?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write(header_str.as_bytes())?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write(b"\n")?;
        
        Ok(())
    }
    
    pub fn load_header(&mut self) -> Result<()> {
        let mut header_str = String::new();
        self.reader.borrow_mut().deref_mut().read_line(&mut header_str)?;
        
        let header: SaveFileHeaders = serde_json::from_str(header_str.as_str())?;
        self.header = header;
        
        Ok(())
    }

    pub fn load_entry_no_filter(&mut self) -> Result<Option<Arc<SaveFileEntry>>> {
        self.load_entry(|_| true)
    }
    
    pub fn load_entry<F: Fn(&SaveFileEntry) -> bool>(&mut self, filter: F) -> Result<Option<Arc<SaveFileEntry>>> {
        loop {
            let mut entry_str = String::new();
            let count = self.reader.borrow_mut().deref_mut().read_line(&mut entry_str)?;

            if count == 0 {
                return Ok(None);
            }
            
            if count == 1 {
                continue;
            }

            let entry: SaveFileEntry = serde_json::from_str(entry_str.as_str())?;

            if entry.hash.hash_type() != self.header.hash_type && !(entry.file_type == SaveFileEntryType::Other && entry.hash.hash_type() == GeneralHashType::NULL) {
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
    
    pub fn load_all_entries<F: Fn(&SaveFileEntry) -> bool>(&mut self, filter: F) -> Result<()> {
        while let Some(_) = self.load_entry(&filter)? {}
        
        Ok(())
    }

    pub fn load_all_entries_no_filter(&mut self) -> Result<()> {
        self.load_all_entries(|_| true)
    }

    pub fn write_entry(&self, result: &SaveFileEntryV1) -> Result<()> {
        let string = serde_json::to_string(result)?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write(string.as_bytes())?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write("\n".as_bytes())?;
        self.writer.borrow_mut().deref_mut().flush()?;
        Ok(())
    }

    pub fn write_entry_ref(&self, result: &SaveFileEntryV1Ref) -> Result<()> {
        let string = serde_json::to_string(result)?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write(string.as_bytes())?;
        *self.written_bytes.borrow_mut() += self.writer.borrow_mut().deref_mut().write("\n".as_bytes())?;
        self.writer.borrow_mut().deref_mut().flush()?;
        Ok(())
    }
    
    pub fn empty_file_by_hash(&mut self) {
        self.file_by_hash.clear();
        self.file_by_hash.shrink_to_fit();
    }
    
    pub fn empty_file_by_path(&mut self) {
        self.file_by_path.clear();
        self.file_by_path.shrink_to_fit();
    }

    pub fn empty_entry_list(&mut self) {
        self.all_entries.clear();
        self.all_entries.shrink_to_fit();
    }

    pub fn get_written_bytes(&self) -> usize {
        *self.written_bytes.borrow()
    }
    
    pub fn flush(&self) -> std::io::Result<()> {
        self.writer.borrow_mut().deref_mut().flush()
    }
}
