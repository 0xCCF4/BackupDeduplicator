use std::path::PathBuf;
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
}
