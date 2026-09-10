use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("CTspmGmuCqbs5UT4DYiJSYXeoSu4SQkpRuws2Mfv4saN");

#[program]
pub mod token_mover {
    use super::*;

    /// Transfer `amount` tokens from `source_token` to `destination_token` via
    /// Token-2022's `transfer_checked`, automatically running whatever transfer
    /// hook is configured on the mint.
    ///
    /// Challenge 4: the hook program and its extra accounts are passed as
    /// `remaining_accounts` (hook program first) so this instruction stays
    /// generic and works with any hook, not just this assignment's.
    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        let source      = ctx.accounts.source_token.to_account_info();
        let mint        = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner       = ctx.accounts.owner.to_account_info();
        let token_prog  = ctx.accounts.token_program.to_account_info();

        // The hook program ID is always the first remaining account.
        let hook_program_id = ctx.remaining_accounts[0].key();

        // 1. Build a plain transfer_checked instruction.
        let mut ix = spl_token_2022::instruction::transfer_checked(
            token_prog.key,
            source.key,
            mint.key,
            destination.key,
            owner.key,
            &[],
            amount,
            ctx.accounts.mint.decimals,
        )?;

        // 2. Start the account-info list in the same order as the instruction accounts.
        let mut infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
        ];

        // 3. Append the hook's extra accounts (reads the ExtraAccountMetaList PDA
        //    and appends exactly what the hook declared — in this case the rate_limit PDA).
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

        // 4. Invoke Token-2022, which will call the hook.
        invoke(&ix, &infos)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,
    /// Source token account — must be owned by `owner` and belong to `mint`.
    #[account(mut, token::mint = mint, token::authority = owner)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    /// Destination token account — must belong to the same `mint`.
    #[account(mut, token::mint = mint)]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    // The hook program, extra_account_meta_list PDA, and rate_limit PDA come in
    // as remaining_accounts so this program stays hook-agnostic.
}
