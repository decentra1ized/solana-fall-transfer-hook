//! The same transfer_checked CPI as token-mover, but issued from the hook
//! program itself. Kept as an executable demonstration that this cannot work:
//! see `tests/test_reentrancy.rs`.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022::{
    self,
    extension::{transfer_hook, PodStateWithExtensions},
    pod::PodMint,
};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut, token::mint = mint, token::authority = authority)]
    pub source: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler<'info>(
    ctx: Context<'info, Transfer<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let source_info = ctx.accounts.source.to_account_info();
    let mint_info = ctx.accounts.mint.to_account_info();
    let destination_info = ctx.accounts.destination.to_account_info();
    let authority_info = ctx.accounts.authority.to_account_info();

    let hook_program_id = {
        let mint_data = mint_info.try_borrow_data()?;
        let mint_state = PodStateWithExtensions::<PodMint>::unpack(&mint_data)?;
        transfer_hook::get_program_id(&mint_state)
            .ok_or(crate::error::ErrorCode::CustomError)?
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

    // Token-2022 will now invoke the hook - which is this very program, already
    // on the call stack. Solana rejects that.
    invoke(&cpi_instruction, &cpi_account_infos)?;

    Ok(())
}
