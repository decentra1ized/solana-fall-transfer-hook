use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

/// Only the accounts a plain transfer needs.
///
/// The hook program, its ExtraAccountMetaList and whatever accounts the hook
/// asked for are deliberately absent: the caller supplies them as remaining
/// accounts. That keeps this program generic — it works with any transfer hook
/// rather than only the rate limiter in this workspace.
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

/// `'info` is named so the account infos and `ctx.remaining_accounts` share one
/// lifetime, which `add_extra_accounts_for_execute_cpi` requires.
pub fn handler<'info>(
    ctx: Context<'info, TransferWithHook<'info>>,
    amount: u64,
) -> Result<()> {
    let source = ctx.accounts.source_token.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let destination = ctx.accounts.destination_token.to_account_info();
    let owner = ctx.accounts.owner.to_account_info();
    let token_program = ctx.accounts.token_program.to_account_info();

    // The caller names the hook program in the first remaining account rather
    // than this program hard-coding one. It is safe: Token-2022 checks it
    // against the hook stored on the mint, so a wrong program fails the
    // transfer instead of bypassing the hook.
    let hook_program_id = ctx.remaining_accounts[0].key();

    // 1. A plain transfer_checked, exactly as a wallet would send it.
    let mut ix = spl_token_2022::instruction::transfer_checked(
        token_program.key,
        source.key,
        mint.key,
        destination.key,
        owner.key,
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // 2. The infos backing that instruction's accounts, in the same order.
    let mut infos = vec![
        source.clone(),
        mint.clone(),
        destination.clone(),
        owner.clone(),
        token_program.clone(),
    ];

    // 3. Append whatever the hook declared in its ExtraAccountMetaList. This
    //    resolves the rate limit PDA from its seeds without us knowing them.
    add_extra_accounts_for_execute_cpi(
        &mut ix,
        &mut infos,
        &hook_program_id,
        source,
        mint,
        destination,
        owner,
        amount,
        ctx.remaining_accounts,
    )?;

    // 4. Hand it to Token-2022, which runs the hook as part of the transfer.
    invoke(&ix, &infos)?;

    Ok(())
}
