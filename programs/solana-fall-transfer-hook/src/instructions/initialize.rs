use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface::Mint};

use crate::{error::ErrorCode, RateLimit, ANCHOR_DISCRIMINATOR_SIZE};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        // Unique, program-wide rate limit account. See the CHALLENGE note in
        // `init_extra_account_meta.rs` for making this per-mint/per-owner.
        seeds = [b"rate_limit", mint.key().as_ref(), owner.key().as_ref()],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // For the challenge - Ensure the mint is a token-2022 mint by checking its owner (Pass the mint in the context and check its owner.
    // Consider saving the mint in the RateLimit struct if needed for future use.
    ctx.accounts.mint.to_account_info().owner == token_2022::ID.ok_or(ErrorCode::InvalidMint)?; // challenge 1
                                                                                                // Initialize the rate limit account with the authority, mint, max amount, and window start timestamp
    ctx.accounts.rate_limit.set_inner(RateLimit {
        authority: ctx.accounts.payer.key(),
        mint: ctx.accounts.mint.key(), //
        max_amount: RateLimit::MAX_AMOUNT,
        window_start: Clock::get()?.unix_timestamp,
        amount_transferred: 0,
    });

    Ok(())
}
