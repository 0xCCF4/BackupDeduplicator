use std::path::{PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Result};

pub trait LexicalAbsolute {
    fn to_lexical_absolute(&self) -> std::io::Result<PathBuf>;
}

impl LexicalAbsolute for PathBuf {
    fn to_lexical_absolute(&self) -> std::io::Result<PathBuf> {
        // https://internals.rust-lang.org/t/path-to-lexical-absolute/14940
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
