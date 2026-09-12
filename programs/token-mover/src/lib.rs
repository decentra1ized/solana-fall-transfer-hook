use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

// This placeholder ID needs to be replaced with this program's real deployed
// address the first time it's built here - run `anchor keys sync` right
// after `anchor build` and it will rewrite this (and Anchor.toml) to match
// the keypair generated at target/deploy/token_mover-keypair.json.
declare_id!("4SsY9ZLWoBCHxKfvmTgV9y54M5axa6ZUQyMkSSag49X4");

// CHALLENGE 4: a program-initiated transfer that still triggers (and is
// still rate-limited by) a Token-2022 transfer hook.
//
// This has to be its own, separate program - it cannot live inside
// `solana_fall_transfer_hook` itself. If the hook program tried to originate
// this transfer, the call stack would be hook-program -> Token-2022 ->
// hook-program again (Token-2022 CPIs into the hook's `Execute` instruction
// mid-transfer), and Solana's runtime rejects a program appearing twice in
// the same CPI stack with "Cross-program invocation reentrancy not allowed
// for this instruction" - it doesn't matter that Token-2022 sits in between.
// A distinct mover program breaks the cycle: mover -> Token-2022 -> hook is
// three different program ids, so nothing reenters.
#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer<'info>(ctx: Context<'info, Transfer<'info>>, amount: u64) -> Result<()> {
        let decimals = ctx.accounts.mint.decimals;

        // Build the base `TransferChecked` instruction exactly as a client would.
        let mut cpi_instruction = spl_token_2022::instruction::transfer_checked(
            ctx.accounts.token_program.key,
            &ctx.accounts.source.key(),
            &ctx.accounts.mint.key(),
            &ctx.accounts.destination.key(),
            &ctx.accounts.authority.key(),
            &[],
            amount,
            decimals,
        )?;

        let mut cpi_account_infos = vec![
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.authority.to_account_info(),
        ];

        // This program doesn't know at compile time which transfer hook a
        // given mint uses, so the caller passes it in as the *first*
        // remaining account, followed by whatever accounts that hook's own
        // `extra_account_metas()` declares (here: the hook's
        // `extra-account-metas` PDA, and - resolved through that PDA's
        // declared seeds - the target program's per-mint/per-owner
        // `rate_limit` PDA). This keeps the mover generic across any hook,
        // not just this one.
        let hook_program_id = ctx.remaining_accounts[0].key;

        add_extra_accounts_for_execute_cpi(
            &mut cpi_instruction,
            &mut cpi_account_infos,
            hook_program_id,
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            amount,
            ctx.remaining_accounts,
        )?;

        // No re-entrancy risk here: this is the only program on the stack
        // that can ever call this transfer, and it never appears again
        // further down (Token-2022 CPIs into the *hook* program next, not
        // back into this one) - see the module-level comment above.
        invoke(&cpi_instruction, &cpi_account_infos)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    /// The authority of `source`. Must sign, same as any normal transfer.
    pub authority: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub source: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    // remaining_accounts: [hook_program_id, ...whatever that hook's
    // extra_account_metas() declares] - see the handler above.
}
