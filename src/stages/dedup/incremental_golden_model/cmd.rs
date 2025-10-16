use crate::stages::analyze::output::{ConflictingEntry, DupSetFile};
use crate::stages::build::output::HashTreeFileEntryType;
use crate::stages::dedup::golden_model::cmd::MatchingModel;
use crate::stages::dedup::output::{
    DeduplicationAction, DeduplicationActionVersion, DeduplicationActions,
};
use anyhow::{anyhow, Result};
use itertools::Itertools;
use log::{info, warn};
use std::fs;
use std::path::PathBuf;

/// Settings for the incremental dedup-goldenref stage.
///
/// # Fields
/// * `input` - The input analysis file to generation actions for.
/// * `output` - The output actions file to write the actions to.
/// * `matching_model` - How to match files.
/// * `reference_model` - The reference model directory.
/// * `directories` - The directories to remove files from.
pub struct DedupIncrementalGoldenModelSettings {
    /// The input analysis file to dedup.
    pub input: PathBuf,
    /// The output action file to write the dedup actions to.
    pub output: PathBuf,
    /// How to match files.
    pub matching_model: MatchingModel,
    /// The incremental directories
    pub directories: Vec<String>,
    /// Other reference folders
    pub reference_models: Vec<String>,
}

/// Run the dedup command.
///
/// # Arguments
/// * `dedup_settings` - The settings for the dedup command.
pub fn run(dedup_settings: DedupIncrementalGoldenModelSettings) -> Result<()> {
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

    let mut result = DeduplicationActions {
        version: DeduplicationActionVersion::V1,
        actions: vec![],
    };

    let directories_matchers = dedup_settings
        .directories
        .iter()
        .map(|dir| Matcher::new(dedup_settings.matching_model, dir))
        .collect::<Vec<Result<Matcher>>>();

    let reference_matchers = dedup_settings
        .reference_models
        .iter()
        .map(|dir| Matcher::new(dedup_settings.matching_model, dir))
        .collect::<Vec<Result<Matcher>>>();

    if let Some(err) = directories_matchers.iter().find(|m| m.is_err()) {
        if let Err(err) = err {
            return Err(anyhow!("This specified directory is invalid: {:?}", err));
        } else {
            unreachable!()
        }
    }

    if let Some(err) = reference_matchers.iter().find(|m| m.is_err()) {
        if let Err(err) = err {
            return Err(anyhow!(
                "This specified reference directory is invalid: {:?}",
                err
            ));
        } else {
            unreachable!()
        }
    }

    for directory in dedup_settings.directories.iter() {
        println!(" - {}", directory);
    }

    let directories_matchers = directories_matchers
        .iter()
        .map(|m| m.as_ref().unwrap())
        .collect::<Vec<&Matcher>>();

    let reference_matchers = reference_matchers
        .iter()
        .map(|m| m.as_ref().unwrap())
        .collect::<Vec<&Matcher>>();

    for entry in &file.entries {
        let mut relevant_conflicting_entries = entry
            .conflicting
            .iter()
            .filter_map(|entry| {
                if let Some(component) = entry.path.first_component() {
                    let x = directories_matchers
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, m)| m.matches(component.to_string_lossy().as_ref()))
                        .map(|(index, _)| (entry, index));
                    x
                } else {
                    None
                }
            })
            .collect::<Vec<(&ConflictingEntry, usize)>>();

        relevant_conflicting_entries.sort_by_key(|(_, index)| *index);

        let retain_file_in_bounce = relevant_conflicting_entries.last().map(|(entry, _)| *entry);

        let reference_matched = entry
            .conflicting
            .iter()
            .filter(|entry| {
                if let Some(component) = entry.path.first_component() {
                    reference_matchers
                        .iter()
                        .any(|m| m.matches(component.to_string_lossy().as_ref()))
                } else {
                    false
                }
            })
            .collect::<Vec<&ConflictingEntry>>();

        if retain_file_in_bounce.is_none() && reference_matched.is_empty() {
            continue;
        }

        let relevant_conflicting_entries = relevant_conflicting_entries
            .into_iter()
            .map(|(entry, _)| entry)
            .collect_vec();

        let remaining_other_duplicates = entry
            .conflicting
            .iter()
            .filter(|p| {
                !relevant_conflicting_entries.contains(p)
                    || if let Some(retain_file) = retain_file_in_bounce {
                        *p == retain_file && reference_matched.is_empty()
                    } else {
                        false
                    }
            })
            .collect::<Vec<&ConflictingEntry>>();

        if !remaining_other_duplicates.is_empty() {
            // delete
            for file in relevant_conflicting_entries {
                if let Some(retain_file) = retain_file_in_bounce {
                    if file == retain_file {
                        continue;
                    }
                }
                match entry.ftype {
                    HashTreeFileEntryType::File | HashTreeFileEntryType::Symlink => {
                        result.actions.push(DeduplicationAction::RemoveFile {
                            path: file.path.clone(),
                            hash: entry.hash.clone(),
                            remaining_duplicates: remaining_other_duplicates
                                .iter()
                                .map(|p| p.path.clone())
                                .collect_vec(),
                            size: entry.size,
                            modification_time: file.modified,
                        });
                    }
                    HashTreeFileEntryType::Directory => {
                        result.actions.push(DeduplicationAction::RemoveDirectory {
                            path: file.path.clone(),
                            hash: entry.hash.clone(),
                            remaining_duplicates: remaining_other_duplicates
                                .iter()
                                .map(|p| p.path.clone())
                                .collect_vec(),
                            children: entry.size,
                            modification_time: file.modified,
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

    info!("Removing redundant actions");

    let directory_removal_paths =  result.actions.iter().filter_map(|x| if let DeduplicationAction::RemoveDirectory {
        path,..
    }=x {Some(path)} else {None}).collect_vec();

    let mut filtered_output = result.actions.iter().filter(|entry| {
        let mut path = match entry.path().first_component() {
            Some(x) => x.to_path_buf(),
            None => return false,
        };
        while let Some(parent) = path.parent() {
            let found = directory_removal_paths.iter().any(|other| if let Some(other_path) = other.first_component() {
                other_path == parent
            } else {
                false
            });

            if found {
                return false;
            }

            path = parent.to_path_buf();
        }

        true
    });
    result.actions = filtered_output.cloned().collect_vec();

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
