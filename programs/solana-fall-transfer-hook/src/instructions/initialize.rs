use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface::Mint};

use crate::{ANCHOR_DISCRIMINATOR_SIZE, RateLimit, error::ErrorCode};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// The mint this rate limit is being opened for.
    ///
    /// `InterfaceAccount<Mint>` already rejects anything that does not deserialize
    /// as a mint, but it accepts both SPL Token and Token-2022. The handler narrows
    /// that to Token-2022 specifically, since a transfer hook only ever fires for a
    /// Token-2022 mint — a rate limit opened against a legacy mint would sit there
    /// looking valid while never being enforced.
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = payer,
        // One bucket per (mint, owner) rather than one for the whole program.
        // These seeds must match `extra_account_metas()` in
        // `init_extra_account_meta.rs`, the `TransferHook` context, and the test
        // helpers — the runtime resolves the account from the metas at transfer
        // time, so any disagreement shows up as a PDA mismatch.
        seeds = [b"rate_limit", mint.key().as_ref(), payer.key().as_ref()],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // Ensure this is a Token-2022 mint. Checking the owning program is the
    // meaningful test: the transfer hook extension only exists under Token-2022,
    // so a mint owned by the legacy SPL Token program can never invoke this hook.
    require_keys_eq!(
        *ctx.accounts.mint.to_account_info().owner,
        token_2022::ID,
        ErrorCode::InvalidMint
    );

    // Initialize the rate limit account with the authority, mint, max amount, and window start timestamp
    ctx.accounts.rate_limit.set_inner(RateLimit {
        authority: ctx.accounts.payer.key(),
        mint: ctx.accounts.mint.key(),
        max_amount: RateLimit::MAX_AMOUNT,
        window_start: Clock::get()?.unix_timestamp,
        amount_transferred: 0
    });

    Ok(())
}
