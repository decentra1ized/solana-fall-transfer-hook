use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

use crate::error::ErrorCode;

#[derive(Accounts)]
pub struct ProgramTransfer<'info> {
    pub owner: Signer<'info>,
    /// CHECK: Token-2022 mutates this (amount + transferring flag)
    #[account(mut)]
    pub source_token: UncheckedAccount<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    /// CHECK: Token-2022 mutates balance
    #[account(mut)]
    pub destination_token: UncheckedAccount<'info>,
    /// CHECK: ExtraAccountMetaList PDA
    pub extra_account_meta_list: UncheckedAccount<'info>,
    /// CHECK: hook mutates this — do NOT type as Account<RateLimit>
    #[account(mut)]
    pub rate_limit: UncheckedAccount<'info>,
    /// CHECK: this program, so Token-2022 can CPI back in
    pub hook_program: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<ProgramTransfer>, amount: u64) -> Result<()> {
    require_keys_eq!(
        *ctx.accounts.hook_program.key,
        crate::ID,
        ErrorCode::CustomError
    );

    let decimals = ctx.accounts.mint.decimals;

    let mut cpi_ix = anchor_spl::token_2022::spl_token_2022::instruction::transfer_checked(
        ctx.accounts.token_program.key,
        ctx.accounts.source_token.key,
        ctx.accounts.mint.to_account_info().key,
        ctx.accounts.destination_token.key,
        ctx.accounts.owner.key,
        &[],
        amount,
        decimals,
    )?;

    let mut cpi_infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    add_extra_accounts_for_execute_cpi(
        &mut cpi_ix,
        &mut cpi_infos,
        ctx.program_id,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        &[
            ctx.accounts.hook_program.to_account_info(),
            ctx.accounts.extra_account_meta_list.to_account_info(),
            ctx.accounts.rate_limit.to_account_info(),
        ],
    )?;

    anchor_lang::solana_program::program::invoke(&cpi_ix, &cpi_infos)?;
    Ok(())
}
