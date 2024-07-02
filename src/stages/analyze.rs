/// Contains the output data structures for the analyze stage.
pub mod output {
    mod dupset_file;

    pub use dupset_file::*;
}

/// Contains the cli command implementation for the analyze command.
pub mod cmd;
mod worker;

/// Contains the intermediary data structures for the analyze stage.
pub mod intermediary_analysis_data;
