use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut, token::mint = mint, token::authority = owner)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint)]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub owner: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

/// Move tokens through Token-2022 without calling this program recursively.
/// Any transfer-hook accounts supplied by the caller are forwarded to the CPI.
pub fn handler<'info>(
    ctx: Context<'info, Transfer<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let cpi_accounts = token_interface::TransferChecked {
        from: ctx.accounts.source_token.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.destination_token.to_account_info(),
        authority: ctx.accounts.owner.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts)
        .with_remaining_accounts(ctx.remaining_accounts.to_vec());
    token_interface::transfer_checked(cpi_ctx, amount, decimals)
}
