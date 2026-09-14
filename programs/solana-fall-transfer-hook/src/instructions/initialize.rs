use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface::Mint};

use crate::{ANCHOR_DISCRIMINATOR_SIZE, RateLimit, error::ErrorCode};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    // Declared above `rate_limit` on purpose: Anchor resolves account fields top
    // to bottom, so `mint` has to exist before the seeds below can reference it.
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = payer,
        // One rate limit per (mint, owner) pair. `payer` is the owner whose
        // transfers this bucket governs - each holder initializes their own.
        // These seeds are mirrored in `init_extra_account_meta.rs`,
        // `transfer_hook.rs` and the test helpers; all of them must agree.
        seeds = [b"rate_limit", mint.key().as_ref(), payer.key().as_ref()],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // `InterfaceAccount<Mint>` deserializes mints from either token program, so
    // on its own it would happily accept a legacy SPL Token mint. Legacy mints
    // cannot carry a transfer hook, which would leave a rate limit account that
    // nothing ever enforces. Narrow it to Token-2022 by checking the owner.
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
