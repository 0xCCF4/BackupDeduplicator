use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use anyhow::{anyhow, Result};
use crate::stages::dedup::output::DeduplicationActions;

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
            "delete"|"d" => Ok(ExecuteActionType::DeleteDuplicates),
            "move"|"m" => Ok(ExecuteActionType::MoveDuplicates),
            _ => Err(anyhow!("Invalid action: {}. Possible values are 'delete' and 'move'", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteAction {
    DeleteDuplicates,
    MoveDuplicates {
        folder_name: String
    },
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
    
    for x in file.actions {
        println!(" - {:?}", x);
    }
    
    Ok(())
}