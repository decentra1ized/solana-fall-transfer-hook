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
        // Challenge 3: The rate limit PDA is now derived per mint and per owner.
        // Seeds must match those used in `initialize.rs` and `transfer_hook.rs`.
        //
        // At transfer time, Token-2022 resolves extra-account PDAs by account
        // index in the ExecuteInstruction layout (NOT by name):
        //   0 = source_token
        //   1 = mint          ← Seed::AccountKey { index: 1 }
        //   2 = destination_token
        //   3 = owner         ← Seed::AccountKey { index: 3 }
        //   4 = extra_account_meta_list
        ExtraAccountMeta::new_with_seeds(
            &[
                Seed::Literal { bytes: b"rate_limit".to_vec() },
                Seed::AccountKey { index: 1 },   // mint
                Seed::AccountKey { index: 3 },   // owner
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