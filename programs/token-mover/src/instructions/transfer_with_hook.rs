use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::{
    token_2022::spl_token_2022::{
        extension::{transfer_hook::TransferHook, BaseStateWithExtensions, PodStateWithExtensions},
        instruction::transfer_checked,
        pod::PodMint,
    },
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

use crate::error::ErrorCode;

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    // The hook program, its ExtraAccountMetaList and the accounts that list
    // resolves to (here, the rate limit PDA) arrive as REMAINING ACCOUNTS.
    // They cannot be named in this struct: which accounts the hook needs is a
    // property of the mint, read at run time, so a fixed struct could only ever
    // describe one hook.
}

// `Context<'info, T>` — Anchor 1.x has a single lifetime. The four-lifetime form
// (`Context<'_, '_, '_, 'info, T>`) is 0.x and no longer compiles. The `'info`
// binding still matters: `remaining_accounts` is `&'info [AccountInfo<'info>]`,
// and the resolver below hands those infos to the CPI.
pub fn handler<'info>(
    ctx: Context<'info, TransferWithHook<'info>>,
    amount: u64,
) -> Result<()> {
    // 1. The transfer we actually want. At this point it is a plain
    //    `transfer_checked` with four accounts and knows nothing about hooks.
    let mut ix = transfer_checked(
        ctx.accounts.token_program.key,
        &ctx.accounts.source_token.key(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.destination_token.key(),
        &ctx.accounts.owner.key(),
        &[],
        amount,
        ctx.accounts.mint.decimals,
    )?;

    // 2. Their infos, in the order `transfer_checked` just wrote them. This
    //    vector and `ix.accounts` are grown together by step 3 and must stay
    //    aligned — `invoke` matches them positionally.
    let mut account_infos = vec![
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
    ];

    // 3. Which hook to resolve against is read from the mint rather than taken
    //    on the caller's word. A caller who could name the hook program could
    //    name a different one — one that resolves cheaply and approves
    //    everything — and the compliance gate would be whatever they passed in.
    //    Token-2022 reads the same field when it dispatches, so this is also
    //    the only value that can be correct.
    let hook_program_id = {
        let mint_info = ctx.accounts.mint.to_account_info();
        let mint_data = mint_info.try_borrow_data()?;
        let mint_state = PodStateWithExtensions::<PodMint>::unpack(&mint_data)?;
        let hook = mint_state.get_extension::<TransferHook>()?;
        Option::<Pubkey>::from(hook.program_id).ok_or(ErrorCode::NoTransferHook)?
        // `mint_data` is dropped here: `invoke` below fails if any account is
        // still borrowed.
    };

    // 4. Append what the hook needs. This reads the mint's ExtraAccountMetaList,
    //    re-derives every PDA in it from the seed recipes, and pushes the
    //    resolved accounts, then the list itself, then the hook program id.
    //    It finds them in `remaining_accounts` by key, so the caller must pass
    //    them all — but their order there does not matter.
    add_extra_accounts_for_execute_cpi(
        &mut ix,
        &mut account_infos,
        &hook_program_id,
        ctx.accounts.source_token.to_account_info(),
        ctx.accounts.mint.to_account_info(),
        ctx.accounts.destination_token.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        amount,
        ctx.remaining_accounts,
    )?;

    // 5. One CPI. Token-2022 sees the extra accounts on the instruction and
    //    passes them through to the hook's `Execute`.
    invoke(&ix, &account_infos)?;

    Ok(())
}
