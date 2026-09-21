//! Challenge 4: Transfer from a program (Re-entrancy handling)
//!
//! Solana allows a program to call itself directly, but disallows a program
//! from being called again through another program (CPI re-entrancy).
//!
//! If the transfer instruction was hosted within this transfer hook program, the call chain:
//! `solana_fall_transfer_hook` -> `Token-2022 transfer_checked` -> `solana_fall_transfer_hook transfer_hook`
//! would fail with `ReentrancyNotAllowed`.
//!
//! To solve this re-entrancy restriction, the transfer instruction is implemented
//! in a separate program (`token-mover`, located in `programs/token-mover/src/lib.rs`).
//! The call chain is therefore:
//! `token-mover` -> `Token-2022 transfer_checked` -> `solana_fall_transfer_hook transfer_hook`,
//! ensuring no program is invoked re-entrantly.
