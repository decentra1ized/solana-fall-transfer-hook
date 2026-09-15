use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::{token_2022::{spl_token_2022, Token2022}, token_interface::{Mint, TokenAccount, TokenInterface}};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("91YP1gsXZo5HMeFQWAmsRc7F5T2LYgwWvz6dyMyAZ8Yi");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'a>(ctx: Context<'a, TransferWithHook<'a>>, amount: u64, decimals: u8) -> Result<()> {
        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();
        let hook_program_id = *ctx.remaining_accounts.first().ok_or(ErrorCode::MissingHookProgram)?.key;
        let mut ix = spl_token_2022::instruction::transfer_checked(
            &Token2022::id(), source.key, mint.key, destination.key, owner.key, &[], amount, decimals,
        ).map_err(|_| error!(ErrorCode::InvalidInstruction))?;
        let mut infos = vec![source.clone(), mint.clone(), destination.clone(), owner.clone()];
        add_extra_accounts_for_execute_cpi(&mut ix, &mut infos, &hook_program_id, source, mint, destination, owner, amount, &ctx.remaining_accounts[1..])
            .map_err(|_| error!(ErrorCode::InvalidInstruction))?;
        invoke(&ix, &infos).map_err(Into::into)
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

#[error_code]
pub enum ErrorCode {
    #[msg("missing hook program")]
    MissingHookProgram,
    #[msg("invalid instruction")]
    InvalidInstruction,
}
