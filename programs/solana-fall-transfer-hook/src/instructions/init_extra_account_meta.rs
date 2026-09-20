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
        // One rate limit account per (mint, owner) pair. Inside the
        // Execute instruction's account list, mint sits at index 1 and the
        // owner at index 3 (TransferHookInstruction::Execute in the
        // interface crate). These seeds must match initialize.rs and
        // transfer_hook.rs exactly, or the derived address won't line up.
        ExtraAccountMeta::new_with_seeds(
            &[
                Seed::Literal { bytes: b"rate_limit".to_vec() },
                Seed::AccountKey { index: 1 }, // mint
                Seed::AccountKey { index: 3 }, // owner
            ],
            false, // is signer
            true,  // is writable
        )?,
    ])
}

pub fn handler(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
    let extra_account_metas = extra_account_metas()?;

    ExtraAccountMetaList::init::<ExecuteInstruction>(
        &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
        &extra_account_metas
    ).unwrap();

    Ok(())
}