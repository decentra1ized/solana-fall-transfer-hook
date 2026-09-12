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
        // A single, program-wide rate limit account derived only from the
        // "rate_limit" literal seed. Every transfer of every mint by every
        // owner resolves to this one account.
        //
        // CHALLENGE: make the rate limit account deterministic *per mint and
        // per owner* by adding the mint and owner as extra seeds.
        //
        // The seeds here must match the PDA seeds used to create the account
        // in `initialize.rs` and to load it in `transfer_hook.rs` (and the
        // test helpers), so all of them have to be updated together.
        ExtraAccountMeta::new_with_seeds(
            &[
                Seed::Literal { bytes: b"rate_limit".to_vec() },
                Seed::AccountKey { index: 1 },
                Seed::AccountKey { index: 3 },
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