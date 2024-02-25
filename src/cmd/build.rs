use std::cell::RefCell;
use std::ops::{DerefMut};
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

    let root = File::new(build_settings.directory, false, inside_scope, lookup_id);
    let mut root = FileContainer::InMemory(RefCell::new(root));

    analyze_file(&mut root);

    let json_str = serde_json::to_string_pretty(&root)?;
    println!("{}", json_str);

    Ok(())
}

fn analyze_file(file: &mut FileContainer) {
    let inside_scope = |path: &'_ Path| -> bool { true };
    let lookup_id = |id: &'_ HandleIdentifier| -> Result<_, _> { Err(anyhow!("lookup_id")) };

    match file {
        FileContainer::InMemory(ref mut file) => {
            let mut file_borrow = file.borrow_mut();
            match file_borrow.deref_mut() {
                File::Directory(ref mut dir) => {
                    dir.analyze_expand(true, inside_scope, lookup_id);
                    // go through children
                    for child in dir.children.iter() {
                        let mut borrow = child.borrow_mut();
                        analyze_file(&mut borrow);
                    }
                    dir.analyze_collect();
                },
                File::File(ref mut file) => {
                    file.analyze();
                },
                File::Other(_) => { /* no analysis needed */ },
                File::Symlink(ref mut file) => {
                    file.analyze(lookup_id);
                }
            }
        },
        FileContainer::OnDisk(_) => {
            panic!("analyze_file: on disk file");
        },
        FileContainer::DoesNotExist => {
            panic!("analyze_file: does not exist");
        },
    }

}
