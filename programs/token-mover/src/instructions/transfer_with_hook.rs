use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,
    #[account(mut, token::mint = mint, token::authority = owner)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint)]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler<'info>(
    ctx: Context<'info, TransferWithHook<'info>>,
    amount: u64,
) -> Result<()> {
    let source_info = ctx.accounts.source_token.to_account_info();
    let mint_info = ctx.accounts.mint.to_account_info();
    let destination_info = ctx.accounts.destination_token.to_account_info();
    let owner_info = ctx.accounts.owner.to_account_info();

    let hook_program_id = ctx.remaining_accounts[0].key;

    // 1. Build transfer_checked instruction
    let mut ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.source_token.key(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.destination_token.key(),
        &ctx.accounts.owner.key(),
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )
    .map_err(|_| error!(ErrorCode::InvalidInstruction))?;

    // 2. Account infos in same order as instruction accounts
    let mut infos = vec![
        source_info.clone(),
        mint_info.clone(),
        destination_info.clone(),
        owner_info.clone(),
    ];

    // 3. Append hook extra accounts
    add_extra_accounts_for_execute_cpi(
        &mut ix,
        &mut infos,
        hook_program_id,
        source_info.clone(),
        mint_info.clone(),
        destination_info.clone(),
        owner_info.clone(),
        amount,
        ctx.remaining_accounts,
    )
    .map_err(|_| error!(ErrorCode::InvalidInstruction))?;

    // 4. Invoke Token-2022
    invoke(&ix, &infos)?;

    Ok(())
}

#[error_code]
pub enum ErrorCode {
    #[msg("Failed to build transfer instruction")]
    InvalidInstruction,
}
