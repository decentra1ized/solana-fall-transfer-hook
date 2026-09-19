use anchor_lang::{
    prelude::*,
    solana_program::program::invoke,
};
use anchor_spl::{
    token_2022::{spl_token_2022, Token2022},
    token_interface::{Mint, TokenAccount},
};
use solana_fall_transfer_hook::RateLimit;
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("8FsuiZQnk5QbnsjuzhbbPBKHhTLKZ5Yf3R3Lt7V6Nyon");

#[program]
pub mod transfer_helper {
    use super::*;

    pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
        let source_info = ctx.accounts.source_token.to_account_info();
        let mint_info = ctx.accounts.mint.to_account_info();
        let destination_info = ctx.accounts.destination_token.to_account_info();
        let owner_info = ctx.accounts.owner.to_account_info();

        let mut transfer_instruction = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            source_info.key,
            mint_info.key,
            destination_info.key,
            owner_info.key,
            &[],
            amount,
            ctx.accounts.mint.decimals,
        )?;
        let mut transfer_account_infos = vec![
            source_info.clone(),
            mint_info.clone(),
            destination_info.clone(),
            owner_info.clone(),
        ];
        let additional_accounts = [
            ctx.accounts.rate_limit.to_account_info(),
            ctx.accounts.extra_account_meta_list.to_account_info(),
            ctx.accounts.hook_program.to_account_info(),
        ];

        add_extra_accounts_for_execute_cpi(
            &mut transfer_instruction,
            &mut transfer_account_infos,
            &ctx.accounts.hook_program.key(),
            source_info,
            mint_info,
            destination_info,
            owner_info,
            amount,
            &additional_accounts,
        )?;

        invoke(&transfer_instruction, &transfer_account_infos)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Transfer<'info> {
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
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"rate_limit", mint.key().as_ref(), owner.key().as_ref()],
        bump,
        seeds::program = hook_program.key(),
    )]
    pub rate_limit: Account<'info, RateLimit>,
    /// CHECK: Address and PDA seeds are constrained to the transfer hook program.
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        seeds::program = hook_program.key(),
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    /// CHECK: The address is constrained to the transfer hook program ID.
    #[account(address = solana_fall_transfer_hook::ID)]
    pub hook_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token2022>,
}
