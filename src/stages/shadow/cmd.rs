use log::{error, trace, warn};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

pub fn run(source: PathBuf, target: PathBuf) {
    let metadata = match std::fs::symlink_metadata(&source) {
        Ok(metadata) => metadata,
        Err(err) => {
            error!("Unable to read metadata for {:?}: {}", source, err);
            return;
        }
    };

    if metadata.is_file() || metadata.is_symlink() {
        trace!("{:?} [FILE] -> {:?}", source, target);
        if let Err(err) = fs::hard_link(&source, &target) {
            error!(
                "Unable to hard link file {:?} to {:?}: {}",
                source, target, err
            );
        }
    } else if metadata.is_dir() {
        trace!("{:?} [DIR] -> {:?}", source, target);

        if let Err(err) = std::fs::create_dir(&target) {
            error!("Unable to create directory {:?}: {}", target, err);
            return;
        }

        let entries = match std::fs::read_dir(&source) {
            Ok(entries) => entries,
            Err(err) => {
                error!("Unable to read directory {:?}: {}", source, err);
                return;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    error!("Unable to read entry in directory {:?}: {}", source, err);
                    continue;
                }
            };
            let entry_path = entry.path();
            let file_name = entry.file_name();
            let target_path = target.join(file_name);

            run(entry_path, target_path);
        }

        if let Err(err) = File::open(&target).map(|dir| metadata.modified().map(|time| dir.set_modified(time))) {
            warn!("Unable to set modification time for directory {:?}: {}", target, err);
        }
    } else {
        error!("{:?} Unknown file type", source);
    }
}
