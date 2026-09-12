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

pub fn handler<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
    require!(
        !ctx.remaining_accounts.is_empty(),
        TokenMoverError::MissingHookAccounts
    );

    let source = ctx.accounts.source_token.to_account_info();
    let mint_info = ctx.accounts.mint.to_account_info();
    let destination = ctx.accounts.destination_token.to_account_info();
    let owner = ctx.accounts.owner.to_account_info();
    let decimals = ctx.accounts.mint.decimals;

    // Convention: whoever builds this instruction puts the hook program
    // first among the remaining accounts.
    let hook_program_id = ctx.remaining_accounts[0].key();

    // 1. Build a plain transfer_checked instruction, as if there were no hook.
    let mut ix = spl_token_2022::instruction::transfer_checked(
        ctx.accounts.token_program.key,
        source.key,
        mint_info.key,
        destination.key,
        owner.key,
        &[],
        amount,
        decimals,
    )?;

    // 2. Matching account-info list, same order as the instruction expects.
    let mut infos = vec![
        source.clone(),
        mint_info.clone(),
        destination.clone(),
        owner.clone(),
    ];

    // 3. Ask the interface to resolve and append whatever extra accounts
    //    THIS mint's hook needs, by reading its ExtraAccountMetaList.
    add_extra_accounts_for_execute_cpi(
        &mut ix,
        &mut infos,
        &hook_program_id,
        source,
        mint_info,
        destination,
        owner,
        amount,
        ctx.remaining_accounts,
    )?;

    // 4. Invoke: Token-2022 moves balances, then itself CPIs into the hook.
    invoke(&ix, &infos)?;

    Ok(())
}

#[error_code]
pub enum TokenMoverError {
    #[msg("No transfer-hook accounts were supplied")]
    MissingHookAccounts,
}
