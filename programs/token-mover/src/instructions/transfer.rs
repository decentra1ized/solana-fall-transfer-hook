use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
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

/// The lifetime has to be named: the extra hook accounts are read out of
/// `ctx.remaining_accounts`, which must live as long as the account infos the
/// CPI is invoked with.
pub fn handler<'info>(
    ctx: Context<'info, TransferWithHook<'info>>,
    amount: u64,
) -> Result<()> {
    // 1. The plain transfer, before any hook accounts are attached.
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

    // 2. The four accounts the transfer itself needs, in interface order.
    let mut infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    // 3. Let the interface append whatever the hook declared — this program
    //    never hard-codes the hook or its PDAs. The hook program itself is the
    //    first remaining account; the rest are resolved from the mint's extra
    //    account meta list.
    let hook_program = ctx
        .remaining_accounts
        .first()
        .ok_or(ErrorCode::AccountNotEnoughKeys)?;

    add_extra_accounts_for_execute_cpi(
        &mut ix,
        &mut infos,
        hook_program.key,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.remaining_accounts,
    )?;

    // 4. The owner signed the outer transaction, so no seeds are needed here.
    anchor_lang::solana_program::program::invoke(&ix, &infos)?;

    Ok(())
}
