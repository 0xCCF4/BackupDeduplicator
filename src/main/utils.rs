use std::env;
use std::path::PathBuf;
use crate::utils::LexicalAbsolute;

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

#[derive(Debug, Clone, Copy)]
pub enum ParsePathKind {
    Direct,
    AbsoluteExisting,
    AbsoluteNonExisting,
}

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
