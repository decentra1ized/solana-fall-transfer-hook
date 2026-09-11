use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

/// Accounts required to transfer Token-2022 tokens through this program.
///
/// The hook-specific accounts are not listed here because different
/// Token-2022 mints can use different transfer hooks. They arrive through
/// `ctx.remaining_accounts` and are resolved automatically.
#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    /// The wallet that owns the source tokens and approves the transfer.
    pub owner: Signer<'info>,

    /// The owner's token account that sends the tokens.
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,

    /// The Token-2022 mint being transferred.
    ///
    /// Its Transfer Hook extension identifies which hook program must run.
    pub mint: InterfaceAccount<'info, Mint>,

    /// The token account that receives the transferred tokens.
    #[account(
        mut,
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,

    /// The token program that performs the checked transfer.
    ///
    /// TokenInterface allows Anchor to work with the Token-2022 program.
    pub token_program: Interface<'info, TokenInterface>,
}

/// Transfers tokens through Token-2022 and allows the mint's hook to run.
///
/// A named `'info` lifetime is required because the hook accounts are
/// borrowed from `ctx.remaining_accounts` and used during the CPI.
pub fn handler<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
    // Step 1: Build a normal Token-2022 transfer_checked instruction.
    //
    // The checked version verifies the mint and its decimal precision.
    let mut transfer_instruction =
        anchor_spl::token_2022::spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.source_token.key(),
            &ctx.accounts.mint.key(),
            &ctx.accounts.destination_token.key(),
            &ctx.accounts.owner.key(),
            &[],
            amount,
            ctx.accounts.mint.decimals,
        )?;

    // Step 2: Start with the four standard accounts required by a
    // Token-2022 transfer.
    let mut transfer_account_infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    // Step 3: The hook program must be the first remaining account.
    //
    // The other remaining accounts contain the Extra Account Meta List
    // and the RateLimit PDA required by our hook.
    let hook_program = ctx
        .remaining_accounts
        .first()
        .ok_or(ErrorCode::AccountNotEnoughKeys)?;

    // Read the mint's hook configuration and Extra Account Meta List,
    // then append every account required by the hook to the CPI.
    //
    // This keeps token-mover generic: it does not hard-code our hook's
    // address or its RateLimit PDA formula.
    add_extra_accounts_for_execute_cpi(
        &mut transfer_instruction,
        &mut transfer_account_infos,
        hook_program.key,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.remaining_accounts,
    )?;

    // Step 4: Send the completed instruction to Token-2022.
    //
    // The owner already signed the outer transaction, so the owner's
    // signature remains available to authorize this CPI.
    anchor_lang::solana_program::program::invoke(&transfer_instruction, &transfer_account_infos)?;

    Ok(())
}
