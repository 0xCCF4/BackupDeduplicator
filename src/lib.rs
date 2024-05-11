#![doc = include_str!("../README.md")]
#![doc = include_str!("lib.md")]

extern crate num_cpus;

pub mod utils;

pub mod pool;

pub mod stages {
    pub mod build;
    pub mod analyze;
    pub mod clean;
}

mod data {
    pub mod path;
    pub mod hash;
    pub mod fileid;
    pub mod compression;
    pub mod archive;
    pub mod copy_stream;
}

pub use data::*;

#[cfg(test)]
mod tests {

}
