use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface::Mint};

use crate::{ANCHOR_DISCRIMINATOR_SIZE, RateLimit, error::ErrorCode};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    /// CHECK: the token account owner this rate limit belongs to; used only as
    /// a seed, and may be a wallet or a PDA owned by another program.
    pub owner: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        // One bucket per (mint, owner). These seeds must stay identical to the
        // ones in `extra_account_metas()` and `transfer_hook.rs`.
        seeds = [b"rate_limit", mint.key().as_ref(), owner.key().as_ref()],
        bump,
        space = ANCHOR_DISCRIMINATOR_SIZE + RateLimit::INIT_SPACE,
    )]
    pub rate_limit: Account<'info, RateLimit>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // `InterfaceAccount<Mint>` accepts either token program, so the interface
    // type alone does not pin this to Token-2022. Only a Token-2022 mint can
    // carry a transfer hook, so a classic SPL mint here would create a rate
    // limit that nothing ever consults.
    require_keys_eq!(
        *ctx.accounts.mint.to_account_info().owner,
        token_2022::ID,
        ErrorCode::InvalidMintOwner
    );

    ctx.accounts.rate_limit.set_inner(RateLimit {
        authority: ctx.accounts.payer.key(),
        mint: ctx.accounts.mint.key(),
        max_amount: RateLimit::MAX_AMOUNT,
        window_start: Clock::get()?.unix_timestamp,
        amount_transferred: 0,
    });

    Ok(())
}
