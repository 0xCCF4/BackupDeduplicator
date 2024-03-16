extern crate num_cpus;

pub mod utils;

mod cmd {
    pub mod build;
}
pub use cmd::*;

pub mod data {
    mod file;
    pub use file::*;
    mod fileid;
    pub use fileid::*;
    mod job;
    pub use job::*;
    mod path;
    pub use path::*;
    mod hash;
    pub use hash::*;
    
    mod save_file;
    pub use save_file::*;
}

mod analysis;

pub mod threadpool;
