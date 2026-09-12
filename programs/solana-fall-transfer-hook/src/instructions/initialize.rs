use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface::Mint};

use crate::{ANCHOR_DISCRIMINATOR_SIZE, RateLimit, error::ErrorCode};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    // The mint this rate limit will apply to. Anchor's `InterfaceAccount<Mint>`
    // already accepts either the legacy Token program or Token-2022, so we
    // additionally check the account's owner in the handler to require Token-2022
    // specifically (the transfer hook this program implements only makes sense
    // for a Token-2022 mint with the transfer-hook extension enabled).
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = payer,
        // Per-mint, per-owner rate limit: `payer` is the owner this account is
        // scoped to. These seeds must match `extra_account_metas()` in
        // `init_extra_account_meta.rs`, the `TransferHook` context in
        // `transfer_hook.rs`, and the test helpers/tests exactly.
        seeds = [b"rate_limit", mint.key().as_ref(), payer.key().as_ref()],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // Ensure the mint is actually owned by the Token-2022 program before we let
    // anyone create a rate limit for it. Without this, a rate limit could be
    // created pointing at a legacy SPL Token mint (or any other account), which
    // would never trigger this transfer hook and would silently enforce
    // nothing.
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
