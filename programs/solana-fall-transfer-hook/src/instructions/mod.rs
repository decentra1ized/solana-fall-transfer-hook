//! Registers the instructions belonging to the transfer-hook program.
//!
//! The actual token transfer will live in a separate `token-mover`
//! program because Solana blocks indirect program re-entrancy.

/// Creates the Extra Account Meta List used by Token-2022.
pub mod init_extra_account_meta;

/// Creates a RateLimit PDA for a mint and owner.
pub mod initialize;

/// Creates a Token-2022 mint with the transfer-hook extension.
pub mod initialize_mint;

/// Validates transfers and updates the owner's rate-limit counter.
pub mod transfer_hook;

// Make the instruction accounts available to the main program module.
pub use init_extra_account_meta::*;
pub use initialize::*;
pub use initialize_mint::*;
pub use transfer_hook::*;
