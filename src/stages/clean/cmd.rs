use crate::hash::GeneralHashType;
use crate::stages::build::output::{HashTreeFile, HashTreeFileEntryType};
use anyhow::{anyhow, Result};
use log::{info, trace, warn};
use std::fs;
use std::path::PathBuf;

/// Settings for the clean stage.
///
/// # Fields
/// * `input` - The input hashtree file to clean.
/// * `output` - The output hashtree file to write the cleaned hashtree to.
/// * `root` - The root path of the original working directory. This is used to resolve relative paths.
/// * `follow_symlinks` - Whether to follow symlinks when checking if files exist.
pub struct CleanSettings {
    /// The input hashtree file to clean.
    pub input: PathBuf,
    /// The output hashtree file to write the cleaned hashtree to.
    pub output: PathBuf,
    /// The root path of the original working directory. This is used to resolve relative paths.
    pub root: Option<String>,
    /// Whether to follow symlinks when checking if files exist.
    pub follow_symlinks: bool,
}

/// Run the clean command.
///
/// # Arguments
/// * `clean_settings` - The settings for the clean command.
pub fn run(clean_settings: CleanSettings) -> Result<()> {
    let mut input_file_options = fs::File::options();
    input_file_options.read(true);
    input_file_options.write(false);

    let mut output_file_options = fs::File::options();
    output_file_options.create(true);
    output_file_options.write(true);

    let input_file = match input_file_options.open(clean_settings.input) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open input file: {}", err));
        }
    };

    let output_file = match output_file_options.open(clean_settings.output) {
        Ok(file) => file,
        Err(err) => {
            return Err(anyhow!("Failed to open output file: {}", err));
        }
    };

    let mut input_buf_reader = std::io::BufReader::new(&input_file);
    let mut output_buf_writer = std::io::BufWriter::new(&output_file);

    let mut save_file = HashTreeFile::new(
        &mut output_buf_writer,
        Some(&mut input_buf_reader),
        GeneralHashType::NULL,
        false,
        true,
        true,
        true,
    );
    save_file.load_header()?;

    // remove duplicates, remove deleted files
    save_file.load_all_entries(|entry| match entry.path.resolve_file() {
        Ok(path) => {
            if !path.exists() {
                return false;
            }

            let metadata = match clean_settings.follow_symlinks {
                true => fs::metadata(path),
                false => fs::symlink_metadata(path),
            };
            let metadata = match metadata {
                Ok(data) => Some(data),
                Err(err) => {
                    warn!("Unable to read metadata of {:?}: {}", entry.path, err);
                    None
                }
            };

            if let Some(metadata) = metadata {
                return if metadata.is_symlink() {
                    entry.file_type == HashTreeFileEntryType::Symlink
                } else if metadata.is_dir() {
                    entry.file_type == HashTreeFileEntryType::Directory
                } else if metadata.is_file() {
                    entry.file_type == HashTreeFileEntryType::File
                } else {
                    entry.file_type == HashTreeFileEntryType::Other
                };
            }

            true
        }
        Err(_err) => true,
    })?;

    // todo filter files deleted from inside archives

    // save results

    info!("Saving results to output file. Dont interrupt this process. It may corrupt the file.");
    save_file.save_header()?;
    for entry in save_file.all_entries.iter() {
        save_file.write_entry(entry)?;
    }

    save_file.flush()?;

    trace!("Truncating output file.");
    fs::File::set_len(&output_file, save_file.get_written_bytes() as u64)?;

    Ok(())
}
