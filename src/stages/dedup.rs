
/// Contains the output data structures for the dedup stage.
pub mod output {
    mod actions;

    pub use crate::stages::dedup::output::actions::*;
}

/// Contains the golden model variant of the dedup stage.
pub mod golden_model {
    /// Contains the cli command implementation for the dedup-golden_model command.
    pub mod cmd;
}
