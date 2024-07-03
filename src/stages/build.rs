/// Contains the output data structures for the build stage.
pub mod output {
    /// Contains conversion functions for the build stage output.
    pub mod converter;
    mod hashtreefile;

    pub use hashtreefile::*;
}

/// Contains the cli command implementation for the build command.
pub mod cmd {
    #[allow(clippy::module_inception)] // private module
    mod cmd;
    /// Contains the job definition for the build command.
    pub mod job;
    /// Contains the worker implementation for the build command.
    pub mod worker;

    pub use cmd::*;
}

/// Contains the intermediary data structures for the build stage.
pub mod intermediary_build_data;
