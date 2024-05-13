use std::io::Read;
use log::error;
use crate::archive::ArchiveType;
use crate::stages::build::cmd::worker::WorkerArgument;
use crate::stages::build::intermediary_build_data::BuildFile;

pub fn worker_run_archive<R: Read + 'static>(input: R, archive_type: ArchiveType, modified: u64, size: u64, id: usize, arg: &mut WorkerArgument) -> Vec<BuildFile> {
    let archive = archive_type.open(input);
    match archive {
        Err(err) => {
            error!("Failed to open archive: {}", err);
            Vec::new()
        },
        Ok(archive) => {
            // todo
            for entry in archive {
                println!("Entry: {:?}", entry);
            }
            Vec::new()
        }
    }
}
