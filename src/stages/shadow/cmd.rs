use std::fs::File;
use std::path::PathBuf;
use log::error;
use std::fs;
use std::io::Write;

pub fn run(source: PathBuf, target: PathBuf) {
    let metadata = match std::fs::metadata(&source) {
        Ok(metadata) => metadata,
        Err(err) => {
            error!("Unable to read metadata for {:?}: {}", source, err);
            return;
        }
    };

    if metadata.is_file() {
        if let Err(err) = fs::hard_link(&source, &target) {
            error!("Unable to hard link file {:?} to {:?}: {}", source, target, err);
        }
    } else if metadata.is_symlink() {
        if let Err(err) = File::create_new(&target).map(|mut file| writeln!(file, "SYMLINK: {:?}", fs::read_link(&source))) {
            error!("Unable to create symlink file {:?} to {:?}: {}", source, target, err);
        }
    } else if metadata.is_dir() {
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
    }
}
