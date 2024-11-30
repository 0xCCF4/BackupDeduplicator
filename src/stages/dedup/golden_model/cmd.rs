use crate::path::FilePath;
use crate::stages::analyze::output::DupSetFile;
use crate::stages::build::output::HashTreeFileEntryType;
use crate::stages::dedup::output::{
    DeduplicationAction, DeduplicationActionVersion, DeduplicationActions,
};
use anyhow::{anyhow, Result};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

/// The matching model to use for deduplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchingModel {
    /// Use regular expressions to match files.
    Regex,
    /// Use plain string matching to match files, anchored at the beginning of the path.
    Plain,
}

impl FromStr for MatchingModel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "regex" => Ok(MatchingModel::Regex),
            "plain" => Ok(MatchingModel::Plain),
            _ => Err(anyhow!(
                "Invalid matching model: {}. Possible values are 'plain', 'regex'",
                s
            )),
        }
    }
}

/// Settings for the dedup-goldenref stage.
///
/// # Fields
/// * `input` - The input analysis file to generation actions for.
/// * `output` - The output actions file to write the actions to.
/// * `matching_model` - How to match files.
/// * `reference_model` - The reference model directory.
/// * `directories` - The directories to remove files from.
pub struct DedupGoldenModelSettings {
    /// The input analysis file to dedup.
    pub input: PathBuf,
    /// The output action file to write the dedup actions to.
    pub output: PathBuf,
    /// How to match files.
    pub matching_model: MatchingModel,
    /// The reference model directory.
    pub reference_model: String,
    /// The directories to removes files from.
    pub directories: Vec<String>,
}

/// Run the dedup command.
///
/// # Arguments
/// * `dedup_settings` - The settings for the dedup command.
pub fn run(dedup_settings: DedupGoldenModelSettings) -> Result<()> {
    let mut input_file_options = fs::File::options();
    input_file_options.read(true);
    input_file_options.write(false);

    let mut output_file_options = fs::File::options();
    output_file_options.create(true);
    output_file_options.write(true);
    output_file_options.truncate(true);

    let input_file = match input_file_options.open(dedup_settings.input) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open input file: {}", err));
        }
    };

    let output_file = match output_file_options.open(dedup_settings.output) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open output file: {}", err));
        }
    };

    let mut input_buf_reader = std::io::BufReader::new(&input_file);

    // parse duplicate file set info from input file
    let file: DupSetFile = match serde_json::from_reader(&mut input_buf_reader) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to read input file: {}", err));
        }
    };

    let golden_directory = match Matcher::new(
        dedup_settings.matching_model,
        &dedup_settings.reference_model,
    ) {
        Ok(reference_model) => reference_model,
        Err(err) => {
            return Err(anyhow!(
                "This specified reference model is invalid: {}",
                err
            ));
        }
    };

    let other_directories = dedup_settings
        .directories
        .iter()
        .map(|dir| Matcher::new(dedup_settings.matching_model, dir))
        .collect::<Vec<Result<Matcher>>>();

    if let Some(err) = other_directories.iter().find(|m| m.is_err()) {
        if let Err(err) = err {
            return Err(anyhow!("This specified directory is invalid: {:?}", err));
        } else {
            unreachable!()
        }
    }

    let mut other_directories = other_directories
        .iter()
        .map(|m| m.as_ref().unwrap())
        .collect::<Vec<&Matcher>>();

    if other_directories.is_empty() {
        other_directories.push(Matcher::all_ref());
    }

    let mut result = DeduplicationActions {
        version: DeduplicationActionVersion::V1,
        actions: vec![],
    };

    for entry in file.entries {
        let reference_files = entry
            .conflicting
            .iter()
            .filter(|path| {
                if let Some(component) = path.first_component() {
                    golden_directory.matches(component.to_string_lossy().as_ref())
                } else {
                    false
                }
            })
            .collect::<Vec<&FilePath>>();

        let other_files = entry
            .conflicting
            .iter()
            .filter(|path| !reference_files.contains(path))
            .filter(|path| {
                if let Some(component) = path.first_component() {
                    other_directories
                        .iter()
                        .any(|v| v.matches(component.to_string_lossy().as_ref()))
                } else {
                    false
                }
            })
            .collect::<Vec<&FilePath>>();

        let remaining_duplicates = entry
            .conflicting
            .iter()
            .filter(|p| reference_files.contains(p) || !other_files.contains(p))
            .collect::<Vec<&FilePath>>();

        if !reference_files.is_empty() && !other_files.is_empty() {
            // delete
            for file in other_files {
                match entry.ftype {
                    HashTreeFileEntryType::File | HashTreeFileEntryType::Symlink => {
                        result.actions.push(DeduplicationAction::RemoveFile {
                            path: file.clone(),
                            hash: entry.hash.clone(),
                            remaining_duplicates: remaining_duplicates
                                .clone()
                                .into_iter()
                                .cloned()
                                .collect::<Vec<FilePath>>(),
                            size: entry.size,
                        });
                    }
                    HashTreeFileEntryType::Directory => {
                        result.actions.push(DeduplicationAction::RemoveDirectory {
                            path: file.clone(),
                            hash: entry.hash.clone(),
                            remaining_duplicates: remaining_duplicates
                                .clone()
                                .into_iter()
                                .cloned()
                                .collect::<Vec<FilePath>>(),
                            children: entry.size,
                        });
                    }
                    HashTreeFileEntryType::Other => {
                        warn!("Unknown file type: {:?}", file);
                        break;
                    }
                }
            }
        }
    }

    let mut output_buf_writer = std::io::BufWriter::new(&output_file);

    match serde_json::to_writer(&mut output_buf_writer, &result) {
        Ok(_) => {}
        Err(err) => {
            return Err(anyhow!("Failed to write output file: {}", err));
        }
    }

    Ok(())
}

enum Matcher {
    Regex(regex::Regex),
    Plain(String),
    All,
}

impl Matcher {
    pub fn new(model: MatchingModel, text: &str) -> Result<Self> {
        match model {
            MatchingModel::Regex => Ok(Matcher::Regex(regex::Regex::new(text)?)),
            MatchingModel::Plain => Ok(Matcher::Plain(text.to_string())),
        }
    }

    pub const fn all_ref() -> &'static Self {
        &Matcher::All
    }

    pub fn matches(&self, path: &str) -> bool {
        match self {
            Matcher::Regex(re) => re.is_match(path),
            Matcher::Plain(s) => path.starts_with(s),
            Matcher::All => true,
        }
    }
}
