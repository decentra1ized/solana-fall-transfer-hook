use std::cell::Ref;

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{
        self,
        spl_token_2022::{
            extension::{
                transfer_hook::TransferHookAccount, BaseStateWithExtensions, PodStateWithExtensions,
            },
            pod::PodAccount,
        },
    },
    token_interface::{Mint, TokenAccount},
};

use crate::{RateLimit, ONE_HOUR};

#[derive(Accounts)]
pub struct TransferHook<'info> {
    #[account(
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: source token account owner, can be SystemAccount or PDA owned by another program
    pub owner: UncheckedAccount<'info>,
    /// CHECK: ExtraAccountMetaList Account
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()], 
        bump
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
}

/// This function is called when the transfer hook is executed.
pub fn handler(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
    // Fail this instruction if it is not called from within a transfer hook
    check_is_transferring(&ctx)?;
    require_keys_eq!(
        *ctx.accounts.mint.to_account_info().owner,
        token_2022::ID,
        crate::error::ErrorCode::InvalidMint
    );
    let rate_limit_info = ctx
        .remaining_accounts
        .first()
        .ok_or_else(|| error!(crate::error::ErrorCode::MissingRateLimit))?;
    require!(
        rate_limit_info.is_writable,
        crate::error::ErrorCode::ReadonlyRateLimit
    );
    let (expected, _) = Pubkey::find_program_address(
        &[
            b"rate_limit",
            ctx.accounts.mint.key().as_ref(),
            ctx.accounts.owner.key().as_ref(),
        ],
        ctx.program_id,
    );
    require_keys_eq!(
        rate_limit_info.key(),
        expected,
        crate::error::ErrorCode::InvalidRateLimit
    );
    let mut rate_limit = Account::<RateLimit>::try_from(rate_limit_info)?;
    // If the current window has expired, open a fresh one. The window start
    // is fixed: update() never moves it, so steady traffic cannot keep a
    // window alive forever (see state/rate_limit.rs).
    let current_time = Clock::get()?.unix_timestamp;
    if rate_limit.is_expired(current_time, ONE_HOUR) {
        rate_limit.reset(current_time);
        msg!("Rate limit window expired - opening a new window");
    }

    // Check if the transfer amount exceeds the rate limit
    match rate_limit.limit_exceeded(amount) {
        // If the limit is exceeded, return an error to prevent the transfer from occurring
        true => {
            msg!("Transfer amount exceeds the rate limit");
            return Err(error!(crate::error::ErrorCode::RateLimitExceeded));
        }
        // If the limit is not exceeded, update the rate limit account with the new amount transferred and allow the transfer to proceed
        false => {
            rate_limit.update(amount);
            msg!("Transfer amount is within the rate limit, proceeding with transfer");
        }
    }

    rate_limit.exit(ctx.program_id)?;
    Ok(())
}

/// Checks if the transfer hook is being executed during a transfer operation.
fn check_is_transferring(ctx: &Context<TransferHook>) -> Result<()> {
    let source_token_info = ctx.accounts.source_token.to_account_info();
    let account_data_ref: Ref<&mut [u8]> = source_token_info.try_borrow_data()?;
    let account = PodStateWithExtensions::<PodAccount>::unpack(*account_data_ref)?;
    let account_extension = account.get_extension::<TransferHookAccount>()?;

    // Return a proper error rather than panicking: a panic aborts with an
    // opaque SBF error, while an Anchor error surfaces a clear code and
    // message to the client.
    require!(
        bool::from(account_extension.transferring),
        crate::error::ErrorCode::NotTransferring
    );

    Ok(())
}
