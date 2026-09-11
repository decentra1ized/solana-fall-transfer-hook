//! Registers the instructions belonging to the token-mover program.
//!
//! This program has only one job: transfer Token-2022 tokens while
//! allowing the mint's transfer hook to run.

/// Contains the accounts and handler for the CPI transfer.
pub mod transfer;

// Makes TransferWithHook available to the main lib.rs program module.
pub use transfer::*;
