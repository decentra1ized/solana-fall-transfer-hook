//! A minimal program that moves Token-2022 tokens on behalf of a signer.
//!
//! The point of this program is re-entrancy. The obvious place to put a
//! `transfer_checked` CPI is inside the transfer hook program itself, but that
//! cannot work: Solana forbids a program from appearing twice in the call
//! stack, and the call chain would be
//!
//! ```text
//! hook program -> Token-2022 -> hook program
//! ```
//!
//! Token-2022 invokes the hook as part of the transfer, so the hook program
//! would re-enter itself and the transaction is rejected. Moving the CPI into
//! a *separate* program breaks the cycle:
//!
//! ```text
//! token-mover -> Token-2022 -> hook program
//! ```
//!
//! No program appears twice, and the hook still runs - the rate limit is
//! enforced exactly as it is for a wallet-initiated transfer.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022::{
    self,
    extension::{transfer_hook, PodStateWithExtensions},
    pod::PodMint,
};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("4NfgcnDDmxhQu88rQ4T1bjWBXTZBvCn6begpmk1NAfAm");

#[error_code]
pub enum ErrorCode {
    #[msg("The mint has no transfer hook configured")]
    NoTransferHook,
}

#[program]
pub mod token_mover {
    use super::*;

    pub fn move_tokens<'info>(
        ctx: Context<'info, MoveTokens<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        handle_move_tokens(ctx, amount, decimals)
    }
}

#[derive(Accounts)]
pub struct MoveTokens<'info> {
    #[account(mut, token::mint = mint, token::authority = authority)]
    pub source: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    // remaining_accounts must carry the hook program, its ExtraAccountMetaList
    // and every account that list resolves to (here, the rate limit PDA).
}

pub fn handle_move_tokens<'info>(
    ctx: Context<'info, MoveTokens<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let source_info = ctx.accounts.source.to_account_info();
    let mint_info = ctx.accounts.mint.to_account_info();
    let destination_info = ctx.accounts.destination.to_account_info();
    let authority_info = ctx.accounts.authority.to_account_info();

    // Read the hook program id off the mint rather than taking it from the
    // caller: the mint is the authority on which hook applies to it, so a
    // caller cannot point us at a different (permissive) hook program.
    let hook_program_id = {
        let mint_data = mint_info.try_borrow_data()?;
        let mint_state = PodStateWithExtensions::<PodMint>::unpack(&mint_data)?;
        transfer_hook::get_program_id(&mint_state).ok_or(ErrorCode::NoTransferHook)?
    };

    let mut cpi_instruction = spl_token_2022::instruction::transfer_checked(
        ctx.accounts.token_program.key,
        source_info.key,
        mint_info.key,
        destination_info.key,
        authority_info.key,
        &[],
        amount,
        decimals,
    )?;

    let mut cpi_account_infos = vec![
        source_info.clone(),
        mint_info.clone(),
        destination_info.clone(),
        authority_info.clone(),
    ];

    // Resolve the hook's declared extra accounts from its ExtraAccountMetaList
    // and append them to the CPI. anchor_spl's `transfer_checked` helper cannot
    // be used here: it builds the instruction with only these four accounts and
    // drops any remaining accounts, so Token-2022 would invoke the hook without
    // the rate limit PDA it declared.
    add_extra_accounts_for_execute_cpi(
        &mut cpi_instruction,
        &mut cpi_account_infos,
        &hook_program_id,
        source_info,
        mint_info,
        destination_info,
        authority_info,
        amount,
        ctx.remaining_accounts,
    )?;

    invoke(&cpi_instruction, &cpi_account_infos)?;

    Ok(())
}
