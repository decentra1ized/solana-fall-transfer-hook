use anchor_lang::{prelude::*, solana_program::program::invoke,};
use anchor_spl::{ token_2022::*, token_interface::{TokenAccount, TokenInterface, Mint}};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

use crate::error::ErrorCode;

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

pub fn handle_transfer<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
    let source = ctx.accounts.source_token.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let destination = ctx.accounts.destination_token.to_account_info();
    let owner = ctx.accounts.owner.to_account_info();
    let hook_program_id = ctx.remaining_accounts.first().ok_or_else(|| error!(ErrorCode::MissingHookProgram))?.key();
    let token_program = ctx.accounts.token_program.to_account_info();

    let mut ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.source_token.key(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.destination_token.key(),
        &ctx.accounts.owner.key(),
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;
    let mut infos = vec![
        source.clone(),
        mint.clone(),
        destination.clone(),
        owner.clone(),
    ];
    add_extra_accounts_for_execute_cpi( &mut ix,&mut infos, &hook_program_id, source, mint, destination, owner, amount, ctx.remaining_accounts,)?;

    invoke(&ix, &infos)?;
    Ok(())
}

