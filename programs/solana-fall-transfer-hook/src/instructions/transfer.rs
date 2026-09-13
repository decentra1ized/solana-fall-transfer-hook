use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

#[derive(Accounts)]
pub struct Transfer<'info> {
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
    // The hook program and its extra accounts (this program, the
    // ExtraAccountMetaList and the resolved rate_limit PDA) arrive as
    // `remaining_accounts` rather than named fields, because which accounts a hook
    // needs is a property of the mint, not of this instruction.
}

/// Transfers tokens with `transfer_checked`, driving the mint's transfer hook.
///
/// ## Re-entrancy — the constraint that shapes this instruction
///
/// The obvious implementation does not work, and it is worth being precise about why.
/// If this program CPIs into Token-2022, Token-2022 must then CPI into the mint's
/// transfer hook — which is *this same program*. That gives:
///
///   this program (depth 1) -> Token-2022 (depth 2) -> this program's hook (depth 3)
///
/// Solana's runtime rejects that with `ReentrancyNotAllowed`. The rule is that a
/// program already on the invocation stack may not be re-entered from a deeper frame
/// (only direct self-recursion is permitted), and it is enforced by the runtime, not
/// by Token-2022 — so no amount of account juggling on our side avoids it.
///
/// Note this is purely about *who is on the stack*, not about the hook itself: when a
/// transfer is submitted at top level, Token-2022 sits at depth 1 and invokes the hook
/// at depth 2, and everything works. That is the path the other tests exercise.
///
/// ## What this instruction therefore does
///
/// It performs the `transfer_checked` CPI with the hook's extra accounts correctly
/// resolved and appended, which is the part a caller cannot easily do themselves. The
/// hook program in `remaining_accounts` must be a *different* program from this one —
/// the normal composition case, where a program moves tokens for a mint whose hook
/// belongs to somebody else.
///
/// Pointing it at a mint whose hook is this very program is a legitimate thing to try
/// and it fails cleanly with `ReentrancyNotAllowed` rather than silently skipping the
/// hook. Silently skipping would be the dangerous outcome: a transfer that dodges its
/// own rate limit. Failing loudly is the correct behaviour.
///
/// ## Why the rate limit is not an account here
///
/// This instruction deliberately does not name the rate limit account. The hook loads
/// it through its own `Account<RateLimit>` borrow; holding a second mutable borrow in
/// the outer frame would collide at runtime. It arrives via `remaining_accounts` and is
/// only ever borrowed one frame down.
pub fn handler<'info>(ctx: Context<'info, Transfer<'info>>, amount: u64) -> Result<()> {
    // Build the base transfer_checked instruction. The hook's extra accounts are
    // appended below rather than listed here.
    let mut cpi_instruction = spl_token_2022::instruction::transfer_checked(
        ctx.accounts.token_program.key,
        &ctx.accounts.source_token.key(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.destination_token.key(),
        &ctx.accounts.owner.key(),
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;

    let mut cpi_account_infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    // Resolve the hook's extra accounts from the mint's ExtraAccountMetaList and
    // append them to the CPI. This is what turns the declared seeds in
    // `extra_account_metas()` into the concrete rate_limit PDA for this
    // (mint, owner) pair — the same resolution the off-chain client performs.
    add_extra_accounts_for_execute_cpi(
        &mut cpi_instruction,
        &mut cpi_account_infos,
        &crate::ID,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.remaining_accounts,
    )?;

    anchor_lang::solana_program::program::invoke(&cpi_instruction, &cpi_account_infos)?;

    Ok(())
}
