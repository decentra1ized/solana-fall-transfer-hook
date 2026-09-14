use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::AccountMeta;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::{Token2022, spl_token_2022};
use anchor_spl::token_interface::{Mint, TokenAccount};

declare_id!("699AKEMqaVFsGbJynh8q65thVkefL4VhXK3UR5QFxVkB");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer<'a>(ctx: Context<'a, Transfer<'a>>, amount: u64, decimals: u8) -> Result<()> {
        // Build transfer_checked instruction manually so we can append
        // the hook accounts before invoking
        let mut ix = spl_token_2022::instruction::transfer_checked(
            &Token2022::id(),
            ctx.accounts.source.to_account_info().key,
            ctx.accounts.mint.to_account_info().key,
            ctx.accounts.destination.to_account_info().key,
            ctx.accounts.owner.to_account_info().key,
            &[],
            amount,
            decimals,
        ).map_err(|e| anchor_lang::error::Error::from(e))?;

        // Append remaining accounts (hook program, extra_account_meta_list, rate_limit)
        for account in ctx.remaining_accounts.iter() {
            ix.accounts.push(
                if account.is_writable {
                    AccountMeta::new(*account.key, account.is_signer)
                } else {
                    AccountMeta::new_readonly(*account.key, account.is_signer)
                }
            );
        }

        // Build account infos for invoke
        let mut account_infos = vec![
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.owner.to_account_info(),
        ];
        for account in ctx.remaining_accounts.iter() {
            account_infos.push(account.clone());
        }

        invoke(&ix, &account_infos)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut)]
    pub source: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token2022>,
}
