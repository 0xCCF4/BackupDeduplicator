/// Contains the output data structures for the build stage.
pub mod output {
    mod hashtreefile;

    pub use hashtreefile::*;
}

/// Contains the cli command implementation for the build command.
pub mod cmd {
    #[allow(clippy::module_inception)] // private module
    mod cmd;
    /// Contains the worker implementation for the build command.
    pub mod worker;

    pub mod archive;

    mod planner;

    pub use cmd::*;
}
