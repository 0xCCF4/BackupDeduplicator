use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::Arc;

use anyhow::Result;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::data::{FilePath, GeneralHash, GeneralHashType};
use crate::utils;

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum SaveFileEntryTypeV1 {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SaveFileEntryV1 {
    pub file_type: SaveFileEntryTypeV1,
    pub modified: u64,
    pub hash: GeneralHash,
    pub path: FilePath,
}

#[derive(Debug, Serialize)]
pub struct SaveFileEntryV1Ref<'a> {
    pub file_type: &'a SaveFileEntryTypeV1,
    pub modified: &'a u64,
    pub hash: &'a GeneralHash,
    pub path: &'a FilePath,
}

pub mod converter;

pub struct SaveFile<'a, W, R> where W: Write, R: BufRead {
    pub header: SaveFileHeaders,
    pub file_by_hash: HashMap<GeneralHash, Vec<Arc<SaveFileEntryV1>>>,
    pub file_by_path: HashMap<FilePath, Arc<SaveFileEntryV1>>,
    
    enable_file_by_hash: bool,
    enable_file_by_path: bool,
    
    writer: &'a mut W,
    reader: &'a mut R,
}

impl<'a, W: Write, R: BufRead> SaveFile<'a, W, R> {
    pub fn new(writer: &'a mut W, reader: &'a mut R, enable_file_by_hash: bool, enable_file_by_path: bool) -> Self {
        let time = utils::get_time();
        SaveFile {
            header: SaveFileHeaders {
                version: SaveFileVersion::V1,
                hash_type: GeneralHashType::SHA256,
                creation_date: time,
            },
            file_by_hash: HashMap::new(),
            file_by_path: HashMap::new(),
            enable_file_by_hash,
            enable_file_by_path,
            writer,
            reader,
        }
    }
    
    pub fn save_header(&mut self) -> Result<()> {
        let header_str = serde_json::to_string(&self.header)?;
        self.writer.write(header_str.as_bytes())?;
        self.writer.write(b"\n")?;
        
        Ok(())
    }
    
    pub fn load_header(&mut self) -> Result<()> {
        let mut header_str = String::new();
        self.reader.read_line(&mut header_str)?;
        
        let header: SaveFileHeaders = serde_json::from_str(header_str.as_str())?;
        self.header = header;
        
        Ok(())
    }
    
    pub fn load_entry(&mut self) -> Result<Option<Arc<SaveFileEntryV1>>> {
        loop {
            let mut entry_str = String::new();
            let count = self.reader.read_line(&mut entry_str)?;

            if count == 0 {
                return Ok(None);
            }

            let entry: SaveFileEntryV1 = serde_json::from_str(entry_str.as_str())?;

            let hash = entry.hash.clone();

            if hash.hash_type() != self.header.hash_type && !(entry.file_type == SaveFileEntryTypeV1::Other && entry.hash.hash_type() == GeneralHashType::NULL) {
                warn!("Hash type mismatch ignoring entry: {:?}", entry.path);
                continue;
            }

            let path = entry.path.clone();

            let shared_entry = Arc::new(entry);

            if self.enable_file_by_hash {
                self.file_by_hash.entry(hash).or_insert_with(Vec::new).push(Arc::clone(&shared_entry));
            }

            if self.enable_file_by_path {
                match self.file_by_path.insert(path, Arc::clone(&shared_entry)) {
                    None => {}
                    Some(old) => {
                        warn!("Duplicate entry for path: {:?}", old.path);
                    }
                }
            }

            return Ok(Some(shared_entry))
        }
    }
    
    pub fn load_all_entries(&mut self) -> Result<()> {
        while let Some(_) = self.load_entry()? {}
        
        Ok(())
    }

    pub fn write_entry(&mut self, result: &SaveFileEntryV1) -> Result<()> {
        let string = serde_json::to_string(result)?;
        self.writer.write(string.as_bytes())?;
        self.writer.write("\n".as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn write_entry_ref(&mut self, result: &SaveFileEntryV1Ref) -> Result<()> {
        let string = serde_json::to_string(result)?;
        self.writer.write(string.as_bytes())?;
        self.writer.write("\n".as_bytes())?;
        self.writer.flush()?;
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
}