use std::cell::RefCell;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::rc::{Rc};
use crate::data::common::{FileContainer, GeneralHash};
use anyhow::{anyhow, Result};
use sha2::Digest;

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

    let mut hasher = match hash {
        GeneralHash::SHA256(_) => sha2::Sha256::new(),
    };
    let mut buffer = [0; 1024];
    let mut content_size = 0;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        content_size += bytes_read as u64;
        if bytes_read == 0 {
            break;
        }
        Digest::update(&mut hasher, &buffer[..bytes_read]);
    }

    *hash = match hash {
        GeneralHash::SHA256(_) => GeneralHash::SHA256(hasher.finalize().into()),
    };

    Ok(content_size)
}

pub fn hash_directory<'a>(children: impl Iterator<Item = &'a Rc<RefCell<FileContainer>>>, hash: &mut GeneralHash) -> Result<u64> {
    let mut hasher = match hash {
        GeneralHash::SHA256(_) => sha2::Sha256::new(),
    };

    let mut content_size = 0;

    for child in children {
        content_size += 1;
        match child.borrow().deref() {
            FileContainer::InMemory(file) => {
                Digest::update(&mut hasher, file.get_content_hash().as_bytes());
            },
            FileContainer::OnDisk(_) => {
                Err(anyhow!("Hashing of unloaded file is not supported."))?
            },
            FileContainer::DoesNotExist => {
                Err(anyhow!("Hashing of non-existent file is not supported."))?
            }
        }
    }

    *hash = match hash {
        GeneralHash::SHA256(_) => GeneralHash::SHA256(hasher.finalize().into()),
    };

    Ok(content_size)
}

pub fn hash_path(path: &Path, hash: &mut GeneralHash) -> Result<()> {
    let mut hasher = match hash {
        GeneralHash::SHA256(_) => sha2::Sha256::new(),
    };

    Digest::update(&mut hasher, path.as_os_str().as_encoded_bytes());

    *hash = match hash {
        GeneralHash::SHA256(_) => GeneralHash::SHA256(hasher.finalize().into()),
    };

    Ok(())
}