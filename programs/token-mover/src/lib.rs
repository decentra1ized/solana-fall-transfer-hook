use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("HS9UmJ65Cj6zmnmeB2FW9eJu6RVmQpBHodLeRwacTBMk");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        handler(ctx, amount)
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

pub fn handler<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
    let source = ctx.accounts.source_token.to_account_info();
    let mint = ctx.accounts.mint.to_account_info();
    let destination = ctx.accounts.destination_token.to_account_info();
    let owner = ctx.accounts.owner.to_account_info();
    let token_program = ctx.accounts.token_program.key();
    let decimals = ctx.accounts.mint.decimals;

    // the hook program id comes from the caller: first remaining account
    let hook_program_id = ctx.remaining_accounts[0].key();

    // 1. build a plain transfer_checked
    let mut ix = spl_token_2022::instruction::transfer_checked(

        // TODO: 8 arguments — token program, source, mint, destination, owner, &[], amount, decimals
        &token_program,
        source.key,
        mint.key,
        destination.key,
        owner.key,
        &[],
        amount,
        decimals,)?;

    // 2. account infos, same order as the instruction
    let mut infos = vec![source.clone(), mint.clone(), destination.clone(), owner.clone()];

    // 3. append the hook's accounts from the ExtraAccountMetaList
    add_extra_accounts_for_execute_cpi(
        &mut ix, &mut infos, &hook_program_id,
        source, mint, destination, owner,
        amount, ctx.remaining_accounts,
    )?;

    // 4. call Token-2022
    invoke(&ix, &infos)?;
    Ok(())
}