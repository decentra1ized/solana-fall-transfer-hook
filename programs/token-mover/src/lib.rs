use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

declare_id!("D7afrt4RP27CMJvvX8fwBeQC8z9mP73ohhpnQC5mZufm");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        handler(ctx, amount)
    }
}

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

/// CPI into Token-2022's `transfer_checked` with the transfer hook still
/// enforced.
///
/// The hook program, the extra account meta list and the rate limit account
/// are NOT named accounts here. The caller passes them as `remaining_accounts`
/// (extra accounts after the named ones), which keeps this program generic:
/// it works with any hook, not only this one.
pub fn handler<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
    use anchor_lang::solana_program::program::invoke;
    use anchor_spl::token_2022::spl_token_2022;
    use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

    let source = ctx.accounts.source_token.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let destination = ctx.accounts.destination_token.to_account_info();
    let owner = ctx.accounts.owner.to_account_info();

    // The hook program id comes from the first remaining account. It is safe:
    // Token-2022 checks the hook against the one stored in the mint. Pass the
    // wrong program and the transfer simply fails.
    let hook_program_id = ctx.remaining_accounts[0].key();

    // 1. Build a plain transfer_checked instruction.
    let mut ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        source.key,
        mint.key,
        destination.key,
        owner.key,
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // 2. The account infos that go with it, in the same order as the instruction.
    let mut infos = vec![
        source.clone(),
        mint.clone(),
        destination.clone(),
        owner.clone(),
    ];

    // 3. Append exactly the extra accounts the hook asked for. It reads the
    //    extra account meta list from the remaining accounts and resolves the
    //    rate limit for (mint, owner) into the instruction.
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

    // 4. Invoke Token-2022. It runs the hook, which enforces the rate limit.
    invoke(&ix, &infos)?;

    Ok(())
}