pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use anchor_spl::token_2022::spl_token_2022;
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

pub use constants::*;
pub use instructions::*;
pub use state::*;


declare_id!("GUMTT11HMTVja8CTmLWmDTgq4f2PqqJ3W8tHAdyYdi98");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
    ctx: Context<'info, TransferWithHook<'info>>,
    amount: u64,
) -> Result<()> {
        // handler goes here next
        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();
        let token_program = ctx.accounts.token_program.to_account_info();

        // the caller passes the hook program id as the first remaining account
        let hook_program_id = ctx.remaining_accounts[0].key();


        // 1. Build a plain transfer_checked instruction
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

        // 2. Collect the account infos in the same order as the instruction
        let mut account_infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
        ];

        // 3. Append the hook's extra accounts (reads ExtraAccountMetaList, adds what it asks for)
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

        // 4. Invoke Token-2022 — this triggers the hook as a nested CPI
        invoke(&ix, &account_infos)?;

        Ok(())
    }
}

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
