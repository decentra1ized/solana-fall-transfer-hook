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
    #[account(mut, token::mint = mint, token::authority = owner)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint)]
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

    // The hook's own program ID is passed in by the caller as the first
    // remaining account, since this program has no built-in knowledge of
    // which hook (if any) is attached to a given mint.
    let hook_program_id = ctx.remaining_accounts[0].key();

    // 1. Build a plain transfer_checked instruction.
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

    // 2. List the account infos in the same order as the instruction's accounts.
    let mut infos = vec![source.clone(), mint.clone(), destination.clone(), owner.clone()];

    // 3. Append whatever extra accounts the hook asked for, read from its
    //    ExtraAccountMetaList (passed in via remaining_accounts).
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

    // 4. Invoke Token-2022. It will call back into the hook program itself
    //    (not this one), so no re-entrancy issue arises.
    invoke(&ix, &infos)?;

    Ok(())
}
