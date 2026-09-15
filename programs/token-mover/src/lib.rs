pub mod error;
pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

// A deterministic address, derived as base58(sha256("solana-fall-transfer-hook::token-mover")).
// LiteSVM loads a program at whatever address `add_program` is given, so the tests
// never need the matching secret key — and no key for this address exists. Deploying
// to a cluster would mean generating a keypair and putting its pubkey here instead.
declare_id!("8aY38YHMJG7G3JZ4W9L3xpT1neqmpJyu1Pj4cnMifYmb");

/// A second program, whose only job is to move hooked tokens by CPI.
///
/// It exists because the hook program cannot do this itself. A transfer of a
/// hooked mint makes Token-2022 CPI into the hook program; if the transfer was
/// itself started by that same hook program, the runtime sees it re-entered and
/// aborts with `ReentrancyNotAllowed`. Solana forbids every re-entrant CPI
/// except direct self-recursion, and this is not that case — the cycle runs
/// hook → Token-2022 → hook.
///
/// So the transfer has to originate somewhere else. Anywhere else: this program
/// is deliberately ordinary, and the hook has no idea it exists.
#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        transfer_with_hook::handler(ctx, amount)
    }
}
