use std::io::Read;
use std::path::PathBuf;
use anyhow::{anyhow, Result};
use log::{trace};
use crate::archive::{ArchiveEntry, ArchiveType};
use crate::path::FilePath;
use crate::stages::build::cmd::worker::WorkerArgument;
use crate::stages::build::intermediary_build_data::{BuildFile, BuildOtherInformation};

pub fn worker_run_archive<R: Read>(input: R, path: &PathBuf, archive_type: ArchiveType, id: usize, _arg: &mut WorkerArgument) -> Result<Vec<BuildFile>> {
    let archive = archive_type.open(input)
        .map_err(|err| {
            anyhow!("Failed to open archive: {}", err)
        })?;

    let mut context = Context {
        id,
        path: FilePath::from_realpath(path).new_archive(),
    };
    
    let mut entries = Vec::new();

    /* for entry in archive {
        let entry = entry.map_err(|err| {
            anyhow!("Failed to read archive entry: {}", err)
        })?;
        
        let result = worker_run_entry(entry, &mut context);
        
        entries.push(result);
    }*/
    
    Ok(entries)
}

struct Context {
    id: usize,
    path: FilePath,
}

fn worker_run_entry(entry: ArchiveEntry, context: &mut Context) -> BuildFile {
    trace!("[{}] Processing archive entry: {:?}", context.id, entry.path);
    
    // todo placeholder
    BuildFile::Other(BuildOtherInformation {
        path: context.path.child(entry.path),
        modified: entry.modified,
        content_size: 0,
    })
    
    /*
    
    let stream = BufferCopyStreamReader::with_capacity();

    let archive = is_archive(entry.stream);

    let stream = match archive {
        Err(err) => {
            error!("[{}] Error while opening nested archive {:?}: {}", context.id, entry.path, err);
            return BuildFile::Other(BuildOtherInformation {
                path: context.path.child(entry.path),
                modified: entry.modified,
                content_size: 0,
            });
        },
        Ok(None) => {
            
        }
    }*/
}
