use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface::Mint};

use crate::{ANCHOR_DISCRIMINATOR_SIZE, RateLimit, error::ErrorCode};

/// Challenge 1: mint is now a required account so we can validate it is
/// owned by Token-2022 before creating the rate limit.
///
/// Challenge 3: seeds now include the mint and the payer (owner) so every
/// user gets their own rate limit account per mint. These seeds must match
/// those in `init_extra_account_meta.rs` and `transfer_hook.rs`.
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// Challenge 1: accept the Token-2022 mint. We verify its owner below.
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = payer,
        // Challenge 3: PDA is now unique per (mint, owner/payer) instead of
        // being a single, program-wide account.
        seeds = [b"rate_limit", mint.key().as_ref(), payer.key().as_ref()],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // Challenge 1: reject any account that is not owned by the Token-2022
    // program. This prevents callers from supplying an arbitrary account (e.g.
    // a fake mint or a legacy Token-program mint) when creating a rate limit.
    require!(
        ctx.accounts.mint.to_account_info().owner == &token_2022::ID,
        ErrorCode::InvalidMint
    );

    // Challenge 2: record the mint on the account so the rate limit is
    // unambiguously tied to a specific Token-2022 mint.
    ctx.accounts.rate_limit.set_inner(RateLimit {
        authority: ctx.accounts.payer.key(),
        mint: ctx.accounts.mint.key(),
        max_amount: RateLimit::MAX_AMOUNT,
        window_start: Clock::get()?.unix_timestamp,
        amount_transferred: 0,
    });

    Ok(())
}
