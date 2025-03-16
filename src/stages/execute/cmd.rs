use crate::stages::dedup::output::DeduplicationActions;
use anyhow::{anyhow, Result};
use log::{debug, warn};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

pub struct ExecuteSettings {
    /// The action file
    pub input: PathBuf,
    /// The root folders/files from the analysis
    pub files: Vec<PathBuf>,
    /// Dry-run
    pub dry_run: bool,
    /// The action to take when duplicates are found
    pub action: ExecuteAction,
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
                    return Err(anyhow!(
                        "Could not resolve path: {:?}. Please specify its parent folder.",
                        action.path()
                    ));
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
            println!(" - {resolved_path:?}: {action:?}");
        }
    }

    Ok(())
}
