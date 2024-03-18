use std::fs;
use std::path::PathBuf;
use anyhow::{anyhow, Result};
use log::{info, trace};
use crate::data::SaveFile;

pub struct CleanSettings {
    pub input: PathBuf,
    pub output: PathBuf,
    pub root: Option<String>,
}

pub fn run(
    clean_settings: CleanSettings,
) -> Result<()> {
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

    let mut save_file = SaveFile::new(&mut output_buf_writer, &mut input_buf_reader, false, true, true);
    save_file.load_header()?;

    // STEP 1 remove duplicates

    save_file.load_all_entries()?;

    // STEP 2 remove deleted files

    // todo

    // STEP 3 remove files not in root

    // todo

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