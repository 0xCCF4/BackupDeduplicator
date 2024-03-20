extern crate num_cpus;

pub mod utils;

mod cmd {
    pub mod build;
    pub mod clean;
    pub mod analyze;
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
    
    mod hashtree_save_file;
    pub use hashtree_save_file::*;
}

pub mod main {
    pub mod utils;
}

pub mod threadpool;
