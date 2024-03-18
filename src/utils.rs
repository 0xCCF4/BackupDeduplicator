use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Result};
use crate::data::{File, GeneralHash};

pub trait LexicalAbsolute {
    fn to_lexical_absolute(&self) -> std::io::Result<PathBuf>;
}

impl LexicalAbsolute for PathBuf {
    fn to_lexical_absolute(&self) -> std::io::Result<PathBuf> {
        let mut absolute = if self.is_absolute() {
            PathBuf::new()
        } else {
            std::env::current_dir()?
        };
        for component in self.components() {
            match component {
                std::path::Component::CurDir => {},
                std::path::Component::ParentDir => { absolute.pop(); },
                component @ _ => absolute.push(component.as_os_str()),
            }
        }
        Ok(absolute)
    }
}

pub fn hash_file<T>(mut reader: T, hash: &mut GeneralHash) -> Result<u64>
where T: std::io::Read {

    let mut hasher = hash.hasher();
    let mut buffer = [0; 1024];
    let mut content_size = 0;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        content_size += bytes_read as u64;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    *hash = hasher.finalize();

    Ok(content_size)
}

pub fn hash_directory<'a>(children: impl Iterator<Item = &'a File>, hash: &mut GeneralHash) -> Result<u64> {
    let mut hasher = hash.hasher();

    let mut content_size = 0;

    for child in children {
        content_size += 1;
        hasher.update(child.get_content_hash().as_bytes());
    }

    *hash = hasher.finalize();

    Ok(content_size)
}

pub fn hash_path(path: &Path, hash: &mut GeneralHash) -> Result<()> {
    let mut hasher = hash.hasher();

    hasher.update(path.as_os_str().as_encoded_bytes());

    *hash = hasher.finalize();

    Ok(())
}

pub fn decode_hex(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(anyhow!("Invalid hex length"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16)
            .map_err(|e| anyhow!("Failed to parse hex: {}", e)))
        .collect()
}

pub fn get_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs()).unwrap_or(0)
}