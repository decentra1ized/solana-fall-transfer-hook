use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("4EbpuwXRJNaNGcJMxiXfQiSuJEGvbjtnvPbg6ehsBPDa");

// Why this lives in its own program:
//
// The transfer is a CPI into Token-2022, which in turn CPIs into the hook
// program. If the hook program were the one starting that chain, it would end
// up invoking itself and the runtime aborts the whole transaction with
// ReentrancyNotAllowed. Solana only permits direct self-recursion, not a
// program appearing twice in the same CPI stack. Moving the transfer into a
// separate program keeps the chain flat: mover -> Token-2022 -> hook.
#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();

        // remaining_accounts: [hook program, ...the hook's extra accounts]
        let hook_program_id = ctx
            .remaining_accounts
            .first()
            .ok_or(MoverError::MissingHookProgram)?
            .key();

        let mut ix = spl_token_2022::instruction::transfer_checked(
            ctx.accounts.token_program.key,
            source.key,
            mint.key,
            destination.key,
            owner.key,
            &[],
            amount,
            ctx.accounts.mint.decimals,
        )?;

        let mut infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
            ctx.accounts.token_program.to_account_info(),
        ];

        // Appends the hook program and its ExtraAccountMetaList-resolved accounts
        // to both the instruction and the account infos, so Token-2022 can call
        // the hook without the client having to hand-roll the account list.
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
}

#[error_code]
pub enum MoverError {
    #[msg("The transfer hook program must be the first remaining account")]
    MissingHookProgram,
}
