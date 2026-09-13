use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022;
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
    // Hook program, extra-account-meta list and rate limit are NOT here.
    // The caller passes them as remaining accounts (hook program first).
}

pub fn handler<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
    let source = ctx.accounts.source_token.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let destination = ctx.accounts.destination_token.to_account_info();
    let owner = ctx.accounts.owner.to_account_info();
    let decimals = ctx.accounts.mint.decimals;

    // Token-2022 checks this against the hook stored in the mint,
    // so a wrong program here just makes the transfer fail.
    let hook_program_id = ctx
        .remaining_accounts
        .first()
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .key();

    // 1. Build a plain transfer_checked
    let mut ix = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &source.key(),
        &mint.key(),
        &destination.key(),
        &owner.key(),
        &[],
        amount,
        decimals,
    )?;

    // 2. Account infos, same order as the instruction's accounts
    let mut infos = vec![
        source.clone(),
        mint.clone(),
        destination.clone(),
        owner.clone(),
    ];

    // 3. Append hook program + meta list + whatever the hook asked for
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

    // 4. Invoke Token-2022 (owner's signature is forwarded automatically)
    invoke(&ix, &infos)?;
    Ok(())
}
