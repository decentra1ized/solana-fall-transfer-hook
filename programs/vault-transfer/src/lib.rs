//! Moves hooked tokens from inside a program.
//!
//! # Why this is a separate program
//!
//! The obvious approach — have the hook program CPI `transfer_checked` itself —
//! cannot work. The call chain would be
//!
//! ```text
//! hook_program -> Token-2022 -> hook_program
//! ```
//!
//! and Solana forbids direct re-entrancy: a program already on the call stack
//! may not be invoked again. The transfer would be rejected by the runtime
//! before the `transferring` flag ever mattered.
//!
//! Splitting the caller out gives three distinct programs
//!
//! ```text
//! vault_transfer -> Token-2022 -> hook_program
//! ```
//!
//! so nothing re-enters. Token-2022 still sets `transferring` around its CPI
//! into the hook, so the hook's own `check_is_transferring` guard passes and
//! the rate limit is enforced exactly as it is for a wallet-initiated transfer.

use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_token_2022::extension::{transfer_hook, StateWithExtensions};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("A5mkGNv33E7DFsWL522Gwa6i6yhDGn7b3bATbkPaLdZB");

pub const VAULT_SEED: &[u8] = b"vault";

#[program]
pub mod vault_transfer {
    use super::*;

    /// Sends `amount` out of the vault's token account.
    ///
    /// The hook's extra accounts (the hook program, its meta list and the rate
    /// limit PDA) arrive in `remaining_accounts` and are forwarded untouched —
    /// this program never needs to know what they mean, only that Token-2022
    /// will hand them to the hook.
    // The named lifetime ties `remaining_accounts` to the accounts struct.
    // Without it the compiler treats them as two independent lifetimes and
    // `InterfaceAccount` is invariant over 'info, so it cannot unify them.
    pub fn send<'info>(ctx: Context<'info, Send<'info>>, amount: u64) -> Result<()> {
        let mint_key = ctx.accounts.mint.key();
        let bump = [ctx.bumps.vault];
        let signer_seeds: &[&[u8]] = &[VAULT_SEED, mint_key.as_ref(), &bump];

        // Which program the hook is: read it from the mint's TLV rather than
        // hardcoding it, exactly as Token-2022 does. The mint decides.
        let mint_info = ctx.accounts.mint.to_account_info();
        let hook_program_id = {
            let data = mint_info.try_borrow_data()?;
            let state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&data)?;
            transfer_hook::get_program_id(&state)
        };

        let source = ctx.accounts.vault_ata.to_account_info();
        let destination = ctx.accounts.destination.to_account_info();
        let authority = ctx.accounts.vault.to_account_info();

        let mut ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            source.key,
            mint_info.key,
            destination.key,
            authority.key,
            &[],
            amount,
            ctx.accounts.mint.decimals,
        )?;
        let mut infos = vec![
            source.clone(),
            mint_info.clone(),
            destination.clone(),
            authority.clone(),
        ];

        // The step Anchor's `transfer_checked` skips. It appends the hook's
        // resolved accounts to the instruction metas, not just to the infos —
        // without this, Token-2022 cannot see them and fails with
        // MissingAccount before the hook ever runs.
        if let Some(hook_program_id) = hook_program_id {
            add_extra_accounts_for_execute_cpi(
                &mut ix,
                &mut infos,
                &hook_program_id,
                source,
                mint_info,
                destination,
                authority,
                amount,
                ctx.remaining_accounts,
            )?;
        }

        anchor_lang::solana_program::program::invoke_signed(&ix, &infos, &[signer_seeds])?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Send<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: PDA authority over the vault's token account; it signs by seeds.
    #[account(seeds = [VAULT_SEED, mint.key().as_ref()], bump)]
    pub vault: UncheckedAccount<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut, token::mint = mint, token::authority = vault)]
    pub vault_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, token::mint = mint)]
    pub destination: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
