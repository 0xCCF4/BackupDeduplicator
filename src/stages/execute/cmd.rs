use crate::stages::dedup::output::DeduplicationActions;
use anyhow::{anyhow, Result};
use log::{debug, error, warn};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::SystemTime;

pub struct ExecuteSettings {
    /// The action file
    pub input: PathBuf,
    /// The root folders/files from the analysis
    pub files: Vec<PathBuf>,
    /// Dry-run
    pub dry_run: bool,
    /// The action to take when duplicates are found
    pub action: ExecuteAction,
    /// Skip not found errors
    pub ignore_errors: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteActionType {
    DeleteDuplicates,
    MoveDuplicates,
}

impl FromStr for ExecuteActionType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "delete" | "d" => Ok(ExecuteActionType::DeleteDuplicates),
            "move" | "m" => Ok(ExecuteActionType::MoveDuplicates),
            _ => Err(anyhow!(
                "Invalid action: {}. Possible values are 'delete' and 'move'",
                s
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteAction {
    DeleteDuplicates,
    MoveDuplicates { folder_name: String },
}

pub fn run(execute_settings: ExecuteSettings) -> Result<()> {
    let mut input_file_options = fs::File::options();
    input_file_options.read(true);
    input_file_options.write(false);

    let input_file = match input_file_options.open(execute_settings.input) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open input file: {}", err));
        }
    };

    let mut input_buf_reader = std::io::BufReader::new(&input_file);

    let file: DeduplicationActions = match serde_json::from_reader(&mut input_buf_reader) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to read input file: {}", err));
        }
    };

    let mut map_folders = HashMap::with_capacity(execute_settings.files.len());

    let mut untouchable_files = Vec::new();

    debug!("Resolving paths...");
    for action in &file.actions {
        match action.path().first_component() {
            None => {
                warn!("Could not resolve path: {:?}", action.path());
            }
            Some(first_component) => {
                let mut found = false;

                for resolve_folder in execute_settings.files.iter() {
                    let actual_path = resolve_folder.join(first_component);

                    if actual_path.exists() {
                        map_folders.insert(action, actual_path);
                        found = true;
                        break;
                    }
                }

                if !found {
                    if execute_settings.ignore_errors {
                        warn!(
                            "Could not resolve path: {:?}. Please specify its parent folder.",
                            action.path()
                        )
                    } else {
                        return Err(anyhow!(
                            "Could not resolve path: {:?}. Please specify its parent folder.",
                            action.path()
                        ));
                    }
                }
            }
        }
    }

    debug!("Starting action phase...");

    for action in &file.actions {
        if action.path().len() != 1 {
            warn!(
                "Currently only non-archive files are supported: {:?}",
                action.path()
            );
            continue;
        }

        if let Some(resolved_path) = map_folders.get(action) {
            if action.remaining_duplicates().is_empty() {
                error!(
                    "Tried to delete all the duplicates of a file: {:?}. Skipped!",
                    action.path()
                );
                continue;
            }
            let mut queue = Vec::new();
            queue.extend(
                action
                    .remaining_duplicates()
                    .iter()
                    .filter_map(|v| v.first_component().map(|x|x.clone())));

            while let Some(item) = queue.pop() {
                untouchable_files.push(item.clone());
                if let Some(parent) = item.parent() {
                    queue.push(parent.to_path_buf());
                }
            }

            if untouchable_files.contains(&resolved_path) {
                error!(
                    "Tried to delete a file that was marked as untouchable: {:?}. Skipped!",
                    resolved_path
                );
                continue;
            }

            if execute_settings.dry_run {
                println!(" - {resolved_path:?}: {action:?}");
                continue;
            }

            let metadata = match fs::metadata(resolved_path) {
                Ok(metadata) => metadata,
                Err(err) => {
                    error!(
                        "Failed to get metadata of file: {:?}. Error: {}",
                        resolved_path, err
                    );
                    continue;
                }
            };
            let file_size = if resolved_path.is_dir() {
                let children = match fs::read_dir(resolved_path) {
                    Err(err) => {
                        error!(
                            "Failed to read directory: {:?}. Error: {}",
                            resolved_path, err
                        );
                        continue;
                    }
                    Ok(metadata) => metadata,
                };
                children
                    .filter(|x| {
                        if let ExecuteAction::MoveDuplicates { folder_name } =
                            &execute_settings.action
                        {
                            x.as_ref()
                                .map(|x| {
                                    x.path()
                                        .file_name()
                                        .map(|x| &x.to_string_lossy().to_string() != folder_name)
                                        .unwrap_or(true)
                                })
                                .unwrap_or(true)
                        } else {
                            true
                        }
                    })
                    .count() as u64
            } else {
                metadata.len()
            };
            let modified_result = metadata
                .modified()
                .map(|time| {
                    time.duration_since(SystemTime::UNIX_EPOCH)
                        .or(Err(anyhow!(
                            "Unable to convert modified date to UNIX_EPOCH"
                        )))
                        .map(|duration| duration.as_secs())
                })
                .unwrap_or_else(|err| {
                    error!(
                        "Error while reading modified date {:?}: {:?}",
                        resolved_path, err
                    );
                    Ok(0)
                });

            let modified_time = match modified_result {
                Ok(time) => time,
                Err(err) => {
                    error!("Error while processing file {:?}: {}", resolved_path, err);
                    0
                }
            };

            if file_size != action.size() {
                println!(
                    "Size mismatch: {:?} {}->{}",
                    resolved_path,
                    action.size(),
                    file_size
                );
                if resolved_path.is_dir() {
                    let children = fs::read_dir(resolved_path).unwrap();
                    for child in children {
                        let child = child.unwrap();
                        let child_path = child.path();
                        println!("   - {child_path:?}");
                    }
                }
                continue;
            }

            if action.modification_time() != modified_time {
                warn!("Modification time mismatch: {:?}", resolved_path);
            }

            if resolved_path
                .iter()
                .any(|component| component.to_string_lossy() == ".git")
            {
                debug!("Skipped git file: {:?}", resolved_path);
                continue;
            }

            match &execute_settings.action {
                ExecuteAction::DeleteDuplicates => {
                    if let Err(err) = fs::remove_file(resolved_path) {
                        error!("Failed to delete file: {:?}. Error: {}", resolved_path, err);
                    }
                }
                ExecuteAction::MoveDuplicates { folder_name } => {
                    let move_folder = match resolved_path.parent() {
                        Some(parent) => parent.join(folder_name),
                        None => {
                            error!("Could not resolve parent folder of: {:?}", resolved_path);
                            continue;
                        }
                    };
                    if !move_folder.exists() {
                        if let Err(err) = fs::create_dir(&move_folder) {
                            error!("Failed to create folder: {:?} - {:?}", move_folder, err);
                            continue;
                        }
                    }
                    if !move_folder.is_dir() {
                        error!("Path is not a folder: {:?}", move_folder);
                        continue;
                    }

                    let file_name = match resolved_path.file_name() {
                        Some(file_name) => file_name,
                        None => {
                            error!("Unable to read file name for {resolved_path:?}");
                            continue;
                        }
                    };
                    if let Err(err) = fs::rename(resolved_path, move_folder.join(file_name)) {
                        error!("Failed to move file: {:?} -> {:?}", resolved_path, err);
                    }
                }
            }
        }
    }

    Ok(())
}
