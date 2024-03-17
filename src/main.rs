use std::{env};
use std::str::FromStr;
use clap::{arg, Parser, Subcommand};
use log::{debug, info, LevelFilter, trace};
use backup_deduplicator::build::BuildSettings;
use backup_deduplicator::data::GeneralHashType;
use backup_deduplicator::utils::LexicalAbsolute;

/// A simple command line tool to deduplicate backups.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// Number of threads
    /// If set, the tool will use the given number of threads for parallel processing.
    /// If not set, the tool will use the number of logical cores on the system.
    #[arg(short, long)]
    threads: Option<usize>,
    /// Dry-run
    /// If set, the tool will not move any files but only print the actions it would take.
    #[arg(short = 'n', long, default_value = "false")]
    dry_run: bool,
    /// Be verbose, if set, the tool will print more information about the actions it takes. Setting the RUST_LOG env var overrides this flag.
    #[arg(short, long, default_value = "false")]
    verbose: bool,
    /// Debug, if set, the tool will print debug information (including debug implies setting verbose). Setting the RUST_LOG env var overrides this flag.
    #[arg(short, long, default_value = "false")]
    debug: bool,
    /// The subcommand to run
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Build a hash-tree for the given directory
    Build {
        /// The directory to analyze
        #[arg()]
        directory: String,
        /// Traverse into archives
        #[arg(short, long)]
        archives: bool,
        /// Follow symlinks, if set, the tool will not follow symlinks
        #[arg(long)]
        follow_symlinks: bool,
        /// Output hash tree to the given file
        #[arg(short, long, default_value = "hash_tree.bdd")]
        output: String,
        /// Absolute paths, if set, the tool will output absolute paths in the hash tree.
        /// If not set, the tool will output relative paths to the current working directory.
        #[arg(long)]
        absolute_paths: bool,
        /// Working directory, if set, the tool will use the current working directory as the base for relative paths.
        #[arg(short, long)]
        working_directory: Option<String>,
        /// Force overwrite, if set, the tool will overwrite the output file if it exists. If not set, the tool will continue an existing analysis
        #[arg(long="overwrite", default_value = "false")]
        recreate_output: bool,
        /// Hash algorithm to use
        #[arg(long="hash", default_value = "sha256")]
        hash_type: String,
    },
    /*
    /// Update a hash-tree with the given directory
    /// This command will update by checking if the file sizes or modification times have changed.
    Update {
        /// The directory to analyze
        #[arg()]
        directory: String,
        /// The hash tree file to update
        #[arg(short, long, default_value = "hash_tree.bdd")]
        input: String,
        /// Traverse into archives
        #[arg(short, long)]
        archives: bool,
        /// Working directory, if set, the tool will use the current working directory as the base for relative paths.
        #[arg(long)]
        working_directory: Option<String>,
    },
    /// Find duplicates and output them as analysis result
    Analyze {
        /// The hash tree file to analyze
        #[arg(short, long, default_value = "hash_tree.bdd")]
        input: String,
        /// Output file for the analysis result
        #[arg(short, long, default_value = "analysis.json")]
        output: String,
    },
    */
}

fn main() {
    let args = Arguments::parse();

    if !env::vars_os().any(|(key, _)| key == "RUST_LOG") {
        let mut log_level = LevelFilter::Warn;
        if args.verbose {
            log_level = LevelFilter::Info;
        }
        if args.debug {
            log_level = LevelFilter::Debug;
        }
        env::set_var("RUST_LOG", format!("{}", log_level));
    }

    env_logger::init();

    trace!("Initializing program");

    if args.dry_run {
        info!("Running in dry-run mode");
    }
    
    if let Some(threads) = args.threads {
        if threads <= 0 {
            eprintln!("Invalid number of threads: {}", threads);
            std::process::exit(exitcode::CONFIG);
        }
        info!("Using {} threads", threads);
    } else {
        info!("Using optimal number of threads");
    }

    match args.command {
        Command::Build {
            directory,
            archives,
            follow_symlinks,
            output,
            absolute_paths,
            working_directory,
            recreate_output,
            hash_type
        } => {
            debug!("Running build command");
            
            // Check hash_type
            
            let hash_type = match GeneralHashType::from_str(hash_type.as_str()) {
                Ok(hash) => hash,
                Err(supported) => {
                    eprintln!("Unsupported hash type: {}. The values {} are supported.", hash_type.as_str(), supported);
                    std::process::exit(exitcode::CONFIG);
                }
            };

            // Convert to paths and check if they exist

            let directory = std::path::Path::new(&directory);
            let output = std::path::Path::new(&output);
            let working_directory = working_directory.map(|wd| std::path::PathBuf::from(wd));

            if !directory.exists() {
                eprintln!("Target directory does not exist: {}", directory.display());
                std::process::exit(exitcode::CONFIG);
            }

            // Convert to absolute paths

            trace!("Resolving paths");
            trace!("Canonicalization of directory: {:?}", directory);

            let directory = match directory.canonicalize() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("IO error, could not resolve target directory: {:?}", e);
                    std::process::exit(exitcode::CONFIG);
                }
            };

            trace!("Canonicalization of output: {:?}", output);

            let output = match output.to_path_buf().to_lexical_absolute() {
                Ok(out) => out,
                Err(e) => {
                    eprintln!("IO error, could not resolve output file: {:?}", e);
                    std::process::exit(exitcode::CONFIG);
                }
            };

            trace!("Canonicalization of working directory: {:?}", working_directory);

            let working_directory = working_directory.map(|wd| match wd.canonicalize() {
                Ok(wd) => wd,
                Err(e) => {
                    eprintln!("IO error, could not resolve working directory: {:?}", e);
                    std::process::exit(exitcode::CONFIG);
                }
            });

            trace!("Resolved paths");
            trace!("Checking if output directory exists");

            match output.parent().map(|p| p.exists()) {
                Some(false) => {
                    eprintln!("Output directory does not exist: {}", output.display());
                    std::process::exit(exitcode::CONFIG);
                }
                None => {
                    debug!("Output file does not have a parent directory: {}", output.display());
                    eprintln!("IO error, output file location invalid: {}", output.display());
                    std::process::exit(exitcode::CONFIG);
                }
                _ => {}
            }

            // Change working directory
            trace!("Changing working directory");

            if let Some(working_directory) = working_directory {
                env::set_current_dir(&working_directory).unwrap_or_else(|_| {
                    eprintln!("IO error, could not change working directory: {}", working_directory.display());
                    std::process::exit(exitcode::CONFIG);
                });
            }
            let working_directory = std::env::current_dir().unwrap_or_else(|_| {
                eprintln!("IO error, could not resolve working directory");
                std::process::exit(exitcode::CONFIG);
            }).canonicalize().unwrap_or_else(|_| {
                eprintln!("IO error, could not resolve working directory");
                std::process::exit(exitcode::CONFIG);
            });

            // Convert paths to relative path to working directory

            let directory = directory.strip_prefix(&working_directory).unwrap_or_else(|_| {
                eprintln!("IO error, could not resolve target directory relative to working directory");
                std::process::exit(exitcode::CONFIG);
            });

            info!("Target directory: {:?}", directory);
            info!("Archives: {:?}", archives);
            info!("Follow symlinks: {:?}", follow_symlinks);
            info!("Output: {:?}", output);
            info!("Absolute paths: {:?}", absolute_paths);
            info!("Working directory: {:?}", working_directory);

            // Run the command

            match backup_deduplicator::build::run(BuildSettings {
                directory: directory.to_path_buf(),
                into_archives: archives,
                follow_symlinks,
                output,
                absolute_paths,
                threads: args.threads,
                continue_file: !recreate_output,
                hash_type
            }) {
                Ok(_) => {
                    info!("Build command completed successfully");
                    std::process::exit(exitcode::OK);
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    std::process::exit(exitcode::SOFTWARE);
                }
            }
        }
    }
}