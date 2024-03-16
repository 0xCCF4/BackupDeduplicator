use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::data::{File, FilePath, GeneralHash, GeneralHashType};
use anyhow::Result;
use log::warn;
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

type SharedFile = Arc<File>;

pub struct SaveFile {
    pub header: SaveFileHeaders,
    pub file_by_hash: HashMap<GeneralHash, Vec<SharedFile>>,
    pub file_by_path: HashMap<FilePath, SharedFile>,
}

impl SaveFile {
    pub fn new() -> Self {
        let time = utils::get_time();
        SaveFile {
            header: SaveFileHeaders {
                version: SaveFileVersion::V1,
                hash_type: GeneralHashType::SHA256,
                creation_date: time,
            },
            file_by_hash: HashMap::new(),
            file_by_path: HashMap::new(),
        }
    }
    
    pub fn save_header<T: Write>(&self, writer: &mut T) -> Result<()> {
        let header_str = serde_json::to_string(&self.header)?;
        writer.write(header_str.as_bytes())?;
        writer.write(b"\n")?;
        
        Ok(())
    }
    
    pub fn load_header<T: BufRead>(&mut self, reader: &mut T) -> Result<()> {
        let mut header_str = String::new();
        reader.read_line(&mut header_str)?;
        
        let header: SaveFileHeaders = serde_json::from_str(header_str.as_str())?;
        self.header = header;
        
        Ok(())
    }
    
    pub fn load_entry<T: BufRead>(&mut self, reader: &mut T) -> Result<Option<SharedFile>> {
        let mut entry_str = String::new();
        let count = reader.read_line(&mut entry_str)?;
        
        if count == 0 {
            return Ok(None);
        }
        
        let entry: File = serde_json::from_str(entry_str.as_str())?;
        
        let hash = entry.get_content_hash().clone();
        let path = entry.get_path().clone();
        
        let shared_entry = Arc::new(entry);
        
        self.file_by_hash.entry(hash).or_insert_with(Vec::new).push(Arc::clone(&shared_entry));
        match self.file_by_path.insert(path, Arc::clone(&shared_entry)) {
            None => {}
            Some(old) => {
                warn!("Duplicate entry for path: {:?}", old.get_path());
            }
        }
        
        Ok(Some(shared_entry))
    }
    
    pub fn load_all_entries<T: BufRead>(&mut self, reader: &mut T) -> Result<()> {
        while let Some(_) = self.load_entry(reader)? {}
        
        Ok(())
    }
}