use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta, 
    seeds::Seed,
    state::ExtraAccountMetaList
};
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    payer: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    /// CHECK: ExtraAccountMetaList Account, will be initialized in this instruction
    #[account(
        init,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        space = ExtraAccountMetaList::size_of(extra_account_metas()?.len()).unwrap(),
        payer = payer
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn extra_account_metas() -> Result<Vec<ExtraAccountMeta>> {
    Ok(vec![
        // The rate limit account, derived per mint and per owner so each holder
        // gets their own bucket for each mint instead of sharing one global one.
        //
        // The runtime resolves this PDA at transfer time from the seeds declared
        // here, so they must match `initialize.rs` (where the account is created),
        // `transfer_hook.rs` (where it is loaded) and the test helpers exactly.
        //
        // Seeds here cannot name accounts directly — they reference them by index
        // into the Execute instruction's account list, which the transfer hook
        // interface fixes as:
        //
        //   0. source token account
        //   1. mint
        //   2. destination token account
        //   3. owner / source authority
        //
        // Hence index 1 for the mint and index 3 for the owner. Getting these
        // wrong does not fail to compile — it silently resolves a different PDA,
        // and the transfer fails with a seeds constraint violation instead.
        ExtraAccountMeta::new_with_seeds(
            &[
                Seed::Literal { bytes: b"rate_limit".to_vec() },
                Seed::AccountKey { index: 1 },  // mint
                Seed::AccountKey { index: 3 },  // owner (source authority)
            ],
            false,                                  // is signer
            true,                                   // is writable
        )?,
    ])
}

pub fn handler(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
    // Get the extra account metas for the transfer hook
    let extra_account_metas = extra_account_metas()?;

    // initialize ExtraAccountMetaList account with extra accounts
    ExtraAccountMetaList::init::<ExecuteInstruction>(
        &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
        &extra_account_metas
    ).unwrap();

    Ok(())
}