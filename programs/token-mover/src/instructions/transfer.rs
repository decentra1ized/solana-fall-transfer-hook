use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
        token::token_program = token_program,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,

    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,

    #[account(address = anchor_spl::token_2022::ID)]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
    let mut ix = anchor_spl::token_2022::spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.source_token.key(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.destination_token.key(),
        &ctx.accounts.owner.key(),
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;

    let mut account_infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    let hook_program = ctx
        .remaining_accounts
        .first()
        .ok_or(ErrorCode::AccountNotEnoughKeys)?;

    add_extra_accounts_for_execute_cpi(
        &mut ix,
        &mut account_infos,
        hook_program.key,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.remaining_accounts,
    )?;

    anchor_lang::solana_program::program::invoke(&ix, &account_infos)?;

    Ok(())
}
