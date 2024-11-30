use crate::hash::GeneralHashType;
use crate::stages::build::output::{HashTreeFile, HashTreeFileEntryType};
use anyhow::{anyhow, Result};
use log::{info, trace, warn};
use std::fs;
use std::path::PathBuf;
use crate::stages::analyze::output::DupSetFile;
use crate::stages::dedup::output::{DeduplicationActionVersion, DeduplicationActions};

/// Settings for the dedup-goldenref stage.
///
/// # Fields
/// * `input` - The input analysis file to generation actions for.
/// * `output` - The output actions file to write the actions to.
/// * `reference_model` - The reference model directory.
/// * `directories` - The directories to remove files from.
pub struct DedupGoldenModelSettings {
    /// The input analysis file to dedup.
    pub input: PathBuf,
    /// The output action file to write the dedup actions to.
    pub output: PathBuf,
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

    let file: DupSetFile = match serde_json::from_reader(&mut input_buf_reader) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to read input file: {}", err));
        }
    };
    
    let result = DeduplicationActions {
        version: DeduplicationActionVersion::V1,
        actions: vec![],
    };
    
    for entry in file.entries {
        // todo
        println!(" - {:?}", entry);
    }

    let mut output_buf_writer = std::io::BufWriter::new(&output_file);
    
    match serde_json::to_writer_pretty(&mut output_buf_writer, &result) {
        Ok(_) => {}
        Err(err) => {
            return Err(anyhow!("Failed to write output file: {}", err));
        }
    }

    Ok(())
}
