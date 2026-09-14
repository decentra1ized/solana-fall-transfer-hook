use anchor_lang::prelude::*;
use anchor_spl::token_2022;
use spl_token_2022_interface::instruction::transfer_checked;
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("GGEBzupdAvuzTQvgCq3G9itbAaN29HYHN6dqzxsHJyke");

#[program]
pub mod token_mover {
    use super::*;
pub fn transfer<'info>(
    ctx: Context<'info, Transfer<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        let mut cpi_instruction = transfer_checked(
            &token_2022::ID,
            ctx.accounts.source.key,
            ctx.accounts.mint.key,
            ctx.accounts.destination.key,
            ctx.accounts.owner.key,
            &[],
            amount,
            decimals,
        )?;

        let mut cpi_account_infos = vec![
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.owner.to_account_info(),
        ];

        let hook_program_id = ctx
            .remaining_accounts
            .first()
            .ok_or(ErrorCode::HookProgramNotFound)?
            .key;

        add_extra_accounts_for_execute_cpi(
            &mut cpi_instruction,
            &mut cpi_account_infos,
            hook_program_id,
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            amount,
            ctx.remaining_accounts,
        )?;

        anchor_lang::solana_program::program::invoke(
            &cpi_instruction,
            &cpi_account_infos,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    /// CHECK: Token-2022 validates this account.
    #[account(mut)]
    pub source: UncheckedAccount<'info>,

    /// CHECK: Token-2022 validates this account.
    pub mint: UncheckedAccount<'info>,

    /// CHECK: Token-2022 validates this account.
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,

    /// CHECK: Token-2022 validates this account.
    pub owner: UncheckedAccount<'info>,

    /// CHECK: Token-2022 program.
    pub token_program: UncheckedAccount<'info>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Transfer hook program was not provided")]
    HookProgramNotFound,
}
