use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("Bfgs9odmqq3pXuPrLZPHfaKXifXUy9RJsLJfxAjgzpx1");

#[program]
pub mod token_mover {
    use super::*;

    /// Move tokens with a Token-2022 `transfer_checked` CPI, forwarding the
    /// transfer-hook accounts so the rate limit is enforced.
    ///
    /// The transfer hook must run in a *separate* program from the one issuing
    /// the transfer: Token-2022 re-enters the hook program during the transfer,
    /// so if this mover were the hook itself it would be a re-entrant call into
    /// an already-borrowed account. Keeping the mover distinct from the hook
    /// avoids that.
    ///
    /// `remaining_accounts` must be, in order:
    ///   [0] the transfer-hook program
    ///   [1] the mint's ExtraAccountMetaList (validation) account
    ///   [2] the rate_limit PDA (writable)
    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();
        let token_program = ctx.accounts.token_program.to_account_info();

        // The hook program id is supplied as the first remaining account.
        let hook_program_id = ctx.remaining_accounts[0].key();

        // Base transfer_checked instruction: source, mint, destination, owner.
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

        // Account infos the CPI needs but the helper does not add itself: the
        // four transfer accounts plus the token program being invoked.
        let mut account_infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
            token_program.clone(),
        ];

        // Resolve the mint's ExtraAccountMetaList and append the hook's extra
        // accounts (the rate_limit PDA), the validation account and the hook
        // program to both the instruction and the info list.
        add_extra_accounts_for_execute_cpi(
            &mut ix,
            &mut account_infos,
            &hook_program_id,
            source,
            mint,
            destination,
            owner,
            amount,
            ctx.remaining_accounts,
        )?;

        // owner signed the outer transaction, so its signature carries through.
        invoke(&ix, &account_infos)?;

        Ok(())
    }
}

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
