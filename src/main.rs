use std::{env};
use std::str::FromStr;
use clap::{arg, Parser, Subcommand};
use log::{debug, info, LevelFilter, trace};
use backup_deduplicator::hash::GeneralHashType;
use backup_deduplicator::stages::analyze::cmd::AnalysisSettings;
use backup_deduplicator::stages::{analyze, build, clean};
use backup_deduplicator::stages::build::cmd::BuildSettings;
use backup_deduplicator::stages::clean::cmd::CleanSettings;
use backup_deduplicator::utils;

/// A simple command line tool to deduplicate backups.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// Number of threads
    /// If set, the tool will use the given number of threads for parallel processing.
    /// If not set, the tool will use the number of logical cores on the system.
    #[arg(short, long)]
    threads: Option<usize>,
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
        /* /// Traverse into archives
        #[arg(short, long)]
        archives: bool, */
        /// Follow symlinks, if set, the tool will not follow symlinks
        #[arg(long)]
        follow_symlinks: bool,
        /// Output hash tree to the given file
        #[arg(short, long, default_value = "hash_tree.bdd")]
        output: String,
        /// Absolute paths, if set, the tool will output absolute paths in the hash tree.
        /// If not set, the tool will output relative paths to the current working directory.
        // #[arg(long)]
        // absolute_paths: bool,
        /// Working directory, if set, the tool will use the current working directory as the base for relative paths.
        #[arg(short, long)]
        working_directory: Option<String>,
        /// Force overwrite, if set, the tool will overwrite the output file if it exists. If not set, the tool will continue an existing analysis
        #[arg(long="overwrite", default_value = "false")]
        recreate_output: bool,
        /// Hash algorithm to use
        #[arg(long="hash", default_value = "sha256")]
        hash_type: String,
        /// Disable database clean after run, if set the tool will not clean the database after the creation
        #[arg(long="noclean", default_value = "false")]
        no_clean: bool,
    },
    /// Clean a hash-tree file. Removes all files that are not existing anymore. Removes old file versions.
    Clean {
        /// The hash tree file to clean
        #[arg(short, long, default_value = "hash_tree.bdd")]
        input: String,
        /// The directory to clean
        #[arg(short, long, default_value = "hash_tree.bdd")]
        output: String,
        /// Working directory, if set, the tool will use the current working directory as the base for relative paths.
        #[arg(short, long)]
        working_directory: Option<String>,
        /// Overwrite the output file
        #[arg(long="overwrite", default_value = "false")]
        overwrite: bool,
        /// Root directory, if set remove all files that are not subfiles of this directory
        #[arg(long)]
        root: Option<String>,
        /// Follow symlinks, if set, the tool will not follow symlinks
        #[arg(long)]
        follow_symlinks: bool,
    },
    /// Find duplicates and output them as analysis result
    Analyze {
        /// The hash tree file to analyze
        #[arg(short, long, default_value = "hash_tree.bdd")]
        input: String,
        /// Output file for the analysis result
        #[arg(short, long, default_value = "analysis.json")]
        output: String,
        /// Overwrite the output file
        #[arg(long="overwrite", default_value = "false")]
        overwrite: bool,
    },
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
            // archives,
            follow_symlinks,
            output,
            // absolute_paths,
            working_directory,
            recreate_output,
            hash_type,
            no_clean
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

            let directory = utils::main::parse_path(directory.as_str(), utils::main::ParsePathKind::AbsoluteNonExisting);
            let output = utils::main::parse_path(output.as_str(), utils::main::ParsePathKind::AbsoluteNonExisting);
            let working_directory = working_directory.map(|w| utils::main::parse_path(w.as_str(), utils::main::ParsePathKind::AbsoluteNonExisting));

            if !directory.exists() {
                eprintln!("Target directory does not exist: {}", directory.display());
                std::process::exit(exitcode::CONFIG);
            }

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

            let working_directory = utils::main::change_working_directory(working_directory);

            // Convert paths to relative path to working directory

            let directory = directory.strip_prefix(&working_directory).unwrap_or_else(|_| {
                eprintln!("IO error, could not resolve target directory relative to working directory");
                std::process::exit(exitcode::CONFIG);
            });

            info!("Target directory: {:?}", directory);
            // info!("Archives: {:?}", archives);
            info!("Follow symlinks: {:?}", follow_symlinks);
            info!("Output: {:?}", output);
            // info!("Absolute paths: {:?}", absolute_paths);
            info!("Working directory: {:?}", working_directory);

            // Run the command

            match build::cmd::run(BuildSettings {
                directory: directory.to_path_buf(),
                //into_archives: archives,
                follow_symlinks,
                output: output.clone(),
                // absolute_paths,
                threads: args.threads,
                continue_file: !recreate_output,
                hash_type
            }) {
                Ok(_) => {
                    info!("Build command completed successfully");
                    
                    if !no_clean {
                        info!("Executing clean command");
                        match clean::cmd::run(CleanSettings {
                            input: output.clone(),
                            output: output,
                            root: None,
                            follow_symlinks
                        }) {
                            Ok(_) => {
                                info!("Clean command completed successfully");
                                std::process::exit(exitcode::OK);
                            }
                            Err(e) => {
                                eprintln!("Error: {:?}", e);
                                std::process::exit(exitcode::SOFTWARE);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    std::process::exit(exitcode::SOFTWARE);
                }
            }
        },
        Command::Clean {
            input,
            output,
            overwrite,
            root,
            working_directory,
            follow_symlinks
        } => {
            let input = utils::main::parse_path(input.as_str(), utils::main::ParsePathKind::AbsoluteNonExisting);
            let output = utils::main::parse_path(output.as_str(), utils::main::ParsePathKind::AbsoluteNonExisting);

            // Change working directory
            trace!("Changing working directory");

            utils::main::change_working_directory(working_directory.map(|w| utils::main::parse_path(w.as_str(), utils::main::ParsePathKind::AbsoluteNonExisting)));

            if !input.exists() {
                eprintln!("Input file does not exist: {:?}", input);
                std::process::exit(exitcode::CONFIG);
            }
            
            if output.exists() && !overwrite {
                eprintln!("Output file already exists: {:?}. Set --override to override its content", output);
                std::process::exit(exitcode::CONFIG);
            }
            
            match clean::cmd::run(CleanSettings {
                input,
                output,
                root,
                follow_symlinks
            }) {
                Ok(_) => {
                    info!("Clean command completed successfully");
                    std::process::exit(exitcode::OK);
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    std::process::exit(exitcode::SOFTWARE);
                }
            }
        },
        Command::Analyze {
            input,
            output,
            overwrite
        } => {
            let input = utils::main::parse_path(input.as_str(), utils::main::ParsePathKind::AbsoluteExisting);
            let output = utils::main::parse_path(output.as_str(), utils::main::ParsePathKind::AbsoluteNonExisting);

            if !input.exists() {
                eprintln!("Input file does not exist: {:?}", input);
                std::process::exit(exitcode::CONFIG);
            }
            
            if output.exists() && !overwrite {
                eprintln!("Output file already exists: {:?}. Set --override to override its content", output);
                std::process::exit(exitcode::CONFIG);
            }

            match analyze::cmd::run(AnalysisSettings {
                input,
                output,
                threads: args.threads,
            }) {
                Ok(_) => {
                    info!("Analyze command completed successfully");
                    std::process::exit(exitcode::OK);
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    std::process::exit(exitcode::SOFTWARE);
                }
            }
        },
    }
}
