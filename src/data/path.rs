use std::ffi::OsString;
use std::fmt::Formatter;
use std::path::PathBuf;
use anyhow::{Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub enum ArchiveType {
    Tar,
    Zip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PathTarget {
    File,
    // Archive(ArchiveType),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PathComponent {
    pub path: PathBuf,
    pub target: PathTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct FilePath {
    pub path: Vec<PathComponent>
}

impl FilePath {
    pub fn from_vec(path: Vec<PathComponent>) -> Self {
        FilePath {
            path
        }
    }
    
    pub fn from_path(path: PathBuf, target: PathTarget) -> Self {
        FilePath {
            path: vec![PathComponent {
                path,
                target
            }]
        }
    }
    
    pub fn join(&mut self, path: PathBuf, target: PathTarget) {
        self.path.push(PathComponent {
            path,
            target
        });
    }
    
    pub fn extract_parent(&self, _temp_directory: &PathBuf) {
        todo!("implement")
    }
    
    pub fn delete_parent(&self, _temp_directory: &PathBuf) {
        todo!("implement")
    }
    
    pub fn resolve_file(&self) -> Result<PathBuf> {
        if self.path.len() == 1 {
            match self.path[0].target {
                PathTarget::File => Ok(self.path[0].path.clone()),
            }
        } else {
            todo!("implement")
        }
    }

    pub fn child_real(&self, child_name: OsString) -> FilePath {
        let mut result = FilePath {
            path: self.path.clone()
        };
        
        let component = PathBuf::from(child_name);
        
        match result.path.last_mut() {
            Some(last) => {
                last.path.push(component);
            },
            None => {
                result.path.push(PathComponent {
                    path: component,
                    target: PathTarget::File
                });
            }
        }
        
        return result;
    }
}

impl PartialEq for FilePath {
    fn eq(&self, other: &Self) -> bool {
        self.path.len() == other.path.len() && self.path.iter().zip(other.path.iter()).all(|(a, b)| a == b)
    }
}

impl Eq for FilePath {}

impl std::fmt::Display for FilePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut result = String::new();
        
        let mut first = true; 
        for component in &self.path {
            if first {
                first = false;
            } else {
                result.push_str("| ");
            }
            
            result.push_str(component.path.to_str().unwrap_or_else(|| "<invalid path>"));
        }
        
        write!(f, "{}", result)
    }
}
