use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface::Mint};

use crate::{ANCHOR_DISCRIMINATOR_SIZE, RateLimit, error::ErrorCode};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    // The mint this rate limit is being created for. `InterfaceAccount<Mint>`
    // accepts a mint owned by *either* token program, so it alone does not
    // prove this is a Token-2022 mint - the handler checks the owner.
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = payer,
        // One bucket per (mint, owner). The payer is the owner: each holder
        // calls `initialize` themselves, once per mint they hold.
        // Seeds must match `extra_account_metas()`, `TransferHook` and the
        // test helpers.
        seeds = [b"rate_limit", mint.key().as_ref(), payer.key().as_ref()],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // Only a Token-2022 mint can carry the transfer hook extension, so a rate
    // limit attached to a legacy SPL Token mint could never be enforced - the
    // hook would simply never fire. Reject it at creation time rather than
    // leaving a dead account behind.
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
