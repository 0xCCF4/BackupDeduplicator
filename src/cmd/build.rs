use std::path::PathBuf;
use anyhow::Result;

pub struct BuildSettings {
    pub directory: PathBuf,
    pub into_archives: bool,
    pub follow_symlinks: bool,
    pub output: PathBuf,
    pub absolute_paths: bool,
}

pub fn run(
    build_settings: BuildSettings,
) -> Result<()> {


    Ok(())
}