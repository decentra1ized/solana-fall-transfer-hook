use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("5T5d7SRvsDFD18d4N1XWmeBzSBxwzrtH9imx9K9pE53a");

#[program]
pub mod token_mover {
    use super::*;

    /// Moves tokens on behalf of `owner`, running whatever transfer hook the
    /// mint declares.
    ///
    /// This lives in its own program rather than in the hook program: a hook
    /// that CPIs into Token-2022 would be re-entered through Token-2022 and the
    /// runtime aborts with `ReentrancyNotAllowed`. Here the call chain is
    /// `token-mover -> Token-2022 -> hook`, so no program appears twice.
    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        // remaining_accounts must hold, in order: the hook program, its
        // ExtraAccountMetaList, and every extra account the hook asked for.
        require!(
            !ctx.remaining_accounts.is_empty(),
            MoverError::MissingHookAccounts
        );

        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();
        let decimals = ctx.accounts.mint.decimals;

        // Taken from the caller instead of hard-coded, so this instruction works
        // with any hooked mint rather than just ours. It is safe to trust:
        // Token-2022 compares it against the program id stored in the mint's
        // transfer hook extension and fails the transfer if they disagree.
        let hook_program_id = ctx.remaining_accounts[0].key();

        // 1. Build a plain transfer_checked.
        let mut ix = spl_token_2022::instruction::transfer_checked(
            ctx.accounts.token_program.key,
            source.key,
            mint.key,
            destination.key,
            owner.key,
            &[],
            amount,
            decimals,
        )?;

        // 2. The matching account infos, in the same order as the metas above.
        let mut infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
        ];

        // 3. Append the hook's accounts, resolved from its ExtraAccountMetaList.
        //    For this hook that is the (mint, owner) rate limit PDA.
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

        // 4. Hand it to Token-2022, which invokes the hook before settling.
        invoke(&ix, &infos)?;

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
    // The hook program, its ExtraAccountMetaList and the hook's own accounts are
    // passed as remaining accounts, not named here, so this program stays
    // generic across hooks.
}

#[error_code]
pub enum MoverError {
    #[msg("The hook program and its extra accounts must be passed as remaining accounts")]
    MissingHookAccounts,
}