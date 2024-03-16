use std::ffi::OsString;
use std::path::PathBuf;
use anyhow::{Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum ArchiveType {
    Tar,
    Zip,
}

#[derive(Debug, Clone, Serialize)]
pub enum PathTarget {
    File,
    // Archive(ArchiveType),
}

#[derive(Debug, Clone, Serialize)]
pub struct PathComponent {
    pub path: PathBuf,
    pub target: PathTarget,
}

#[derive(Debug, Clone, Serialize)]
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
