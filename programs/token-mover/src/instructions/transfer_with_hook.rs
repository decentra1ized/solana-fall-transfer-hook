use anchor_lang::{
    prelude::*,
    solana_program::program::invoke,
};
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
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
    decimals: u8,
) -> Result<()> {
    let source = ctx.accounts.source_token.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let destination = ctx.accounts.destination_token.to_account_info();
    let owner = ctx.accounts.owner.to_account_info();

    let hook_program_id = ctx
        .remaining_accounts
        .first()
        .ok_or_else(|| error!(ErrorCode::MissingHookProgram))?
        .key();

    let mut ix = spl_token_2022::instruction::transfer_checked(
        &spl_token_2022::id(),
        &source.key(),
        &mint.key(),
        &destination.key(),
        &owner.key(),
        &[],
        amount,
        decimals,
    )?;

    let mut infos = vec![
        source.clone(),
        mint.clone(),
        destination.clone(),
        owner.clone(),
    ];

    add_extra_accounts_for_execute_cpi(
        &mut ix,
        &mut infos,
        &hook_program_id,
        source.clone(),
        mint.clone(),
        destination.clone(),
        owner.clone(),
        amount,
        ctx.remaining_accounts,
    )?;

    invoke(&ix, &infos)?;

    Ok(())
}

#[error_code]
pub enum ErrorCode {
    #[msg("Hook program account is missing")]
    MissingHookProgram,
}
