//! transfer-proxy — Challenge 4
//!
//! A thin second program that CPIs into Token-2022's `transfer_checked`.
//! Because this program has a *different program ID* from the transfer-hook
//! program, the hook can be invoked by Token-2022 without hitting the
//! Solana runtime's re-entrancy guard (which fires when a program appears
//! twice in the same call stack).
//!
//! Call chain:
//!   test → proxy::proxy_transfer
//!              └─→ Token-2022 transfer_checked  (with hook accounts appended)
//!                       └─→ hook program transfer_hook   ✓ (no re-entrancy)
//!
//! Implementation note: we manually build the `transfer_checked` instruction
//! and append the hook accounts to its account list before calling `invoke`.
//! Using anchor-spl's `token_interface::transfer_checked` with
//! `with_remaining_accounts` only adds the extra accounts to the `invoke`
//! account_infos slice — Token-2022 reads hook program accounts from the
//! *instruction's* `accounts` vector, so they must be added there too.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::AccountMeta, program::invoke};
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

declare_id!("ProXYtransfer111111111111111111111111111111");

#[program]
pub mod transfer_proxy {
    use super::*;

    /// CPI proxy: builds a `transfer_checked` instruction that includes the
    /// hook accounts in its account list (just like the test helper does for
    /// direct transfers), then calls `invoke`.
    ///
    /// The remaining_accounts passed to this instruction must be, in order:
    ///   1. hook program account (read-only)
    ///   2. extra_account_meta_list (read-only)
    ///   3. rate_limit PDA (writable)
    pub fn proxy_transfer<'a>(
        ctx: Context<'a, ProxyTransfer<'a>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        // Start from the canonical transfer_checked instruction.
        let mut ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.source.key(),
            &ctx.accounts.mint.key(),
            &ctx.accounts.destination.key(),
            &ctx.accounts.authority.key(),
            &[],
            amount,
            decimals,
        )
        .map_err(|e| anchor_lang::error::Error::from(e))?;

        // Append the hook extra-accounts to the instruction's account list so
        // Token-2022 can see them when it resolves the hook invocation.
        for acc in ctx.remaining_accounts.iter() {
            ix.accounts.push(if acc.is_writable {
                AccountMeta::new(*acc.key, acc.is_signer)
            } else {
                AccountMeta::new_readonly(*acc.key, acc.is_signer)
            });
        }

        // Collect all account_infos in the same order.
        let mut account_infos = vec![
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.authority.to_account_info(),
        ];
        account_infos.extend_from_slice(ctx.remaining_accounts);

        invoke(&ix, &account_infos)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ProxyTransfer<'info> {
    /// The token account to transfer from.
    #[account(mut)]
    pub source: InterfaceAccount<'info, TokenAccount>,
    /// The mint of the token being transferred.
    pub mint: InterfaceAccount<'info, Mint>,
    /// The token account to transfer into.
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    /// The authority (owner) of the source account.
    pub authority: Signer<'info>,
    /// Token-2022 program.
    pub token_program: Interface<'info, TokenInterface>,
    // Hook accounts (hook program id, extra_account_meta_list, rate_limit PDA)
    // are passed as remaining_accounts and appended to the instruction above.
}
