use std::env;
use std::path::PathBuf;
use crate::utils::LexicalAbsolute;

/// Changes the working directory to the given path.
/// 
/// # Arguments
/// * `working_directory` - The new working directory.
/// 
/// # Returns
/// The new working directory.
/// 
/// # Exit
/// Exits the process if the working directory could not be changed.
pub fn change_working_directory(working_directory: Option<PathBuf>) -> PathBuf {
    match working_directory {
        None => {},
        Some(working_directory) => {
            env::set_current_dir(&working_directory).unwrap_or_else(|_| {
                eprintln!("IO error, could not change working directory: {}", working_directory.display());
                std::process::exit(exitcode::CONFIG);
            });
        }
    }

    env::current_dir().unwrap_or_else(|_| {
        eprintln!("IO error, could not resolve working directory");
        std::process::exit(exitcode::CONFIG);
    }).canonicalize().unwrap_or_else(|_| {
        eprintln!("IO error, could not resolve working directory");
        std::process::exit(exitcode::CONFIG);
    })
}

/// Option how to parse a path.
/// 
/// # See also
/// * [parse_path]
#[derive(Debug, Clone, Copy)]
pub enum ParsePathKind {
    /// Do not post-process the path.
    Direct,
    /// Convert the path to a absolute path. The path must exist.
    AbsoluteExisting,
    /// Convert the path to a absolute path. The path might not exist.
    AbsoluteNonExisting,
}

/// Parse a path from a string.
/// 
/// # Arguments
/// * `path` - The path to parse.
/// * `kind` - How to parse the path.
/// 
/// # Returns
/// The parsed path.
pub fn parse_path(path: &str, kind: ParsePathKind) -> PathBuf {
    let path = std::path::Path::new(path);

    let path = path.to_path_buf();

    let path = match kind {
        ParsePathKind::Direct => path,
        ParsePathKind::AbsoluteExisting => to_lexical_absolute(path, true),
        ParsePathKind::AbsoluteNonExisting => to_lexical_absolute(path, false),
    };

    path
}

/// Convert a path to a absolute path.
/// 
/// # Arguments
/// * `path` - The path to convert.
/// * `exists` - Whether the path must exist.
/// 
/// # Returns
/// The absolute path.
/// 
/// # Exit
/// Exits the process if the path could not be resolved.
pub fn to_lexical_absolute(path: PathBuf, exists: bool) -> PathBuf {
    let path = match exists {
        true => path.canonicalize(),
        false => path.to_lexical_absolute(),
    };

    let path = match path{
        Ok(out) => out,
        Err(e) => {
            eprintln!("IO error, could not resolve output file: {:?}", e);
            std::process::exit(exitcode::CONFIG);
        }
    };
    
    path
}
