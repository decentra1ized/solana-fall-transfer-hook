
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint,TokenAccount,TokenInterface,TransferChecked};

#[derive(Accounts)]

pub struct Transfer<'info>{
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut, token::mint = mint,
    )]

    pub destination_token : InterfaceAccount<'info, TokenAccount>,
    pub mint : InterfaceAccount<'info, Mint>,
    pub owner : Signer<'info>,
    pub token_program : Interface<'info, TokenInterface>
}

pub fn handler(ctx :Context<Transfer>,amount:u64, decimals:u8) ->Result<()>{
    let cpi_accounts = TransferChecked {
        from : ctx.accounts.source_token.to_account_info(),
        mint : ctx.accounts.mint.to_account_info(),
        to : ctx.accounts.destination_token.to_account_info(),
        authority : ctx.accounts.owner.to_account_info()
    };
     let cpi_context = CpiContext::new(
        ctx.accounts.token_program.key(),
        cpi_accounts,
    );

    token_interface::transfer_checked(
        cpi_context,
        amount,
        decimals,
    )
}
