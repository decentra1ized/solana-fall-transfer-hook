use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

/// Accounts required to transfer Token-2022 tokens through this program.
///
/// The hook-specific accounts are not hardcoded here; they arrive through
/// `ctx.remaining_accounts` and are resolved dynamically via `add_extra_accounts_for_execute_cpi`.
#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    /// The wallet that owns the source tokens and authorizes the transfer.
    pub owner: Signer<'info>,

    /// The owner's source token account.
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,

    /// The Token-2022 mint being transferred.
    pub mint: InterfaceAccount<'info, Mint>,

    /// The destination token account.
    #[account(
        mut,
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,

    /// The Token-2022 program.
    pub token_program: Interface<'info, TokenInterface>,
}

/// Transfers tokens through Token-2022 and allows the mint's hook to run.
pub fn handler<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
    let source = ctx.accounts.source_token.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let destination = ctx.accounts.destination_token.to_account_info();
    let owner = ctx.accounts.owner.to_account_info();

    // 1. Build a checked transfer instruction for Token-2022
    let mut transfer_ix = anchor_spl::token_2022::spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        source.key,
        mint.key,
        destination.key,
        owner.key,
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // 2. Base account infos for the transfer
    let mut account_infos = vec![
        source.clone(),
        mint.clone(),
        destination.clone(),
        owner.clone(),
    ];

    // 3. The hook program must be the first remaining account
    let hook_program = ctx
        .remaining_accounts
        .first()
        .ok_or(ErrorCode::AccountNotEnoughKeys)?;

    // 4. Append extra accounts required by the transfer hook
    add_extra_accounts_for_execute_cpi(
        &mut transfer_ix,
        &mut account_infos,
        hook_program.key,
        source,
        mint,
        destination,
        owner,
        amount,
        ctx.remaining_accounts,
    )?;

    // 5. Invoke Token-2022 via CPI. Since token-mover is the caller,
    // when Token-2022 calls solana-fall-transfer-hook it is a fresh call,
    // avoiding the Solana runtime ReentrancyNotAllowed guard.
    anchor_lang::solana_program::program::invoke(&transfer_ix, &account_infos)?;

    Ok(())
}
