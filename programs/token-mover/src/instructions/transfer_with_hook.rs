use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::{
    token_2022::spl_token_2022::{
        self,
        extension::{transfer_hook::TransferHook, BaseStateWithExtensions, PodStateWithExtensions},
        pod::PodMint,
    },
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

use crate::error::ErrorCode;

/// Accounts for a Token-2022 `transfer_checked` made from inside a program.
///
/// The hook program, its `ExtraAccountMetaList` and every account the hook
/// needs (here, the `rate_limit` PDA) arrive as `remaining_accounts`. They are
/// not named here because their number and identity depend on the mint.
#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
        token::token_program = token_program,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler<'info>(
    ctx: Context<'info, TransferWithHook<'info>>,
    amount: u64,
) -> Result<()> {
    // Which program does Token-2022 call back into? The mint's TransferHook
    // extension holds the answer, so the caller does not have to tell us.
    let hook_program_id = read_transfer_hook_program_id(&ctx.accounts.mint.to_account_info())?;

    // 1. Build the plain `transfer_checked` instruction.
    let mut cpi_instruction = spl_token_2022::instruction::transfer_checked(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.source_token.key(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.destination_token.key(),
        &ctx.accounts.owner.key(),
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // 2. List the account infos in the same order as the instruction metas.
    let mut cpi_account_infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    // 3. Append the hook accounts. The helper reads the ExtraAccountMetaList,
    //    resolves each extra account (the rate_limit PDA), and picks the
    //    matching infos out of `remaining_accounts`.
    add_extra_accounts_for_execute_cpi(
        &mut cpi_instruction,
        &mut cpi_account_infos,
        &hook_program_id,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.remaining_accounts,
    )?;

    // 4. Invoke Token-2022. The call chain is:
    //       token-mover -> Token-2022 -> transfer hook program
    //    The hook lives in a different program, so nothing re-enters
    //    token-mover and the runtime's re-entrancy rule is satisfied.
    invoke(&cpi_instruction, &cpi_account_infos)?;

    Ok(())
}

/// Reads the transfer hook program id out of the mint's TransferHook extension.
fn read_transfer_hook_program_id(mint_info: &AccountInfo) -> Result<Pubkey> {
    let mint_data = mint_info.try_borrow_data()?;
    let mint_state = PodStateWithExtensions::<PodMint>::unpack(*mint_data)?;
    let extension = mint_state.get_extension::<TransferHook>()?;

    Option::<Pubkey>::from(extension.program_id).ok_or(error!(ErrorCode::NoTransferHook))
}
