use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use crate::data::common::{File, FileContainer, HandleIdentifier};

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

    let inside_scope = |path: &'_ Path| -> bool { true };
    let lookup_id = |id: &'_ HandleIdentifier| -> Result<_, _> { Err(anyhow!("lookup_id")) };

    let mut root = File::new(build_settings.directory, false, inside_scope, lookup_id);
    if let(File::Directory(ref mut dir)) = root {
        dir.analyze_expand(false, inside_scope, lookup_id);
    }
    let mut root = FileContainer::InMemory(root);

    analyze_file(&mut root);

    let json_str = serde_json::to_string_pretty(&root)?;
    println!("{}", json_str);

    Ok(())
}

fn analyze_file(file: &mut FileContainer) {
    let inside_scope = |path: &'_ Path| -> bool { true };
    let lookup_id = |id: &'_ HandleIdentifier| -> Result<_, _> { Err(anyhow!("lookup_id")) };

    if let FileContainer::InMemory(ref mut file) = file {
        if let File::Directory(ref mut dir) = file {
            dir.analyze_expand(false, inside_scope, lookup_id);
            // go through children
            for child in dir.children.iter() {
                let mut borrow = child.borrow_mut();
                analyze_file(&mut borrow);
            }
            dir.analyze_collect();
        }
        if let File::File(ref mut file) = file {
            file.analyze();
        }
    }


}
