#![doc = include_str!("../README.md")]
#![doc = include_str!("lib.md")]
#![warn(missing_docs)]

extern crate num_cpus;

pub mod utils;

pub mod pool;

pub mod stages {
    pub mod analyze;
    pub mod build;
    pub mod clean;
}

mod data {
    pub mod archive;
    pub mod compression;
    pub mod copy_stream;
    pub mod fileid;
    pub mod hash;
    pub mod path;
}

pub use data::*;

#[cfg(test)]
mod tests {}
