#![doc = include_str!("../README.md")]
#![doc = include_str!("lib.md")]
#![warn(missing_docs)]

extern crate num_cpus;

/// Contains utilities that are used over the whole project and within the main file.
pub mod utils;

/// Contains the thread pool implementation. Responsible to distribute jobs over several threads.
pub mod pool;

/// Contains the implementation of the main commands.
pub mod stages {
    /// Contains the implementation of the analyze command.
    pub mod analyze;
    /// Contains the implementation of the build command.
    pub mod build;
    /// Contains the implementation of the clean command.
    pub mod clean;
}

mod data {
    /// Contains the implementation to interface with different archive formats.
    pub mod archive;
    /// Contains the implementation to interface with different compression formats.
    pub mod compression;
    /// Contains the implementation to read streams while buffering them at the same time.
    pub mod copy_stream;
    /// Contains the implementation to parse unique file ids like inode numbers.
    pub mod fileid;
    /// Contains the implementation to interface with different hashing algorithms.
    pub mod hash;
    /// Contains the implementation of the path data structure.
    pub mod path;
}

pub use data::*;

#[cfg(test)]
mod tests {}
