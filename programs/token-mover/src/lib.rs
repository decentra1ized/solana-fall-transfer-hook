use anchor_lang::prelude::*;

pub mod instructions;

use instructions::*;

declare_id!("EAgQBN9bqCJaWEcHa3ePcrR7TmpgrszJVeYkFhUg4GTU");

#[program]
pub mod token_mover {
    use super::*;
    use anchor_lang::solana_program::program::invoke;
    use anchor_spl::token_2022::spl_token_2022;
    use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        let hook_program_id = *ctx.remaining_accounts[0].key;

        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();

        let mut ix = spl_token_2022::instruction::transfer_checked(
            &spl_token_2022::ID,
            source.key,
            mint.key,
            destination.key,
            owner.key,
            &[],
            amount,
            ctx.accounts.mint.decimals,
        )
        .map_err(|_| error!(TransferError::InvalidTransfer))?;

        let mut infos = vec![source.clone(), mint.clone(), destination.clone(), owner.clone()];

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
        )
        .map_err(Error::from)?;

        invoke(&ix, &infos).map_err(Error::from)?;
        Ok(())
    }
}

#[error_code]
pub enum TransferError {
    #[msg("Token transfer failed")]
    InvalidTransfer,
}
