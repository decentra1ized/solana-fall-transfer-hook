pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

// The public address of Christian's token-mover program.
//
// This matches the public key from:
// target/deploy/token_mover-keypair.json
declare_id!("65DV2Vib2V2caLcf96n5pTSPYsNzAB9TqqWPTXjyeQtZ");

/// A small helper program that transfers Token-2022 tokens through CPI.
///
/// This must be separate from the transfer-hook program.
///
/// An attempted call from the hook program would follow:
/// transfer hook → Token-2022 → same transfer hook
///
/// Solana blocks that indirect re-entrancy.
///
/// This separate program creates the valid path:
/// token mover → Token-2022 → transfer hook
#[program]
pub mod token_mover {
    use super::*;

    /// Transfers Token-2022 tokens while still allowing the mint's
    /// transfer hook to inspect and approve the transaction.
    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        transfer::handler(ctx, amount)
    }
}
