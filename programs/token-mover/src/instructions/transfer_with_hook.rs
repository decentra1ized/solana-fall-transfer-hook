use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use anchor_spl::token_2022::spl_token_2022;
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_transfer_with_hook<'info>(
    ctx: Context<'info, TransferWithHook<'info>>,
    amount: u64,
) -> Result<()> {
    let decimals = ctx.accounts.mint.decimals;

    let mut ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.source_token.key(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.destination_token.key(),
        &ctx.accounts.owner.key(),
        &[],
        amount,
        decimals,
    )?;

    let hook_program_id = ctx.remaining_accounts[0].key();

    let mut account_infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    add_extra_accounts_for_execute_cpi(
        &mut ix,
        &mut account_infos,
        &hook_program_id,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.remaining_accounts,
    )?;

    invoke(&ix, &account_infos)?;

    Ok(())
}