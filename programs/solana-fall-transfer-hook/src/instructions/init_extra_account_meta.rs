use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta, seeds::Seed, state::ExtraAccountMetaList,
};
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

/// Accounts needed to create the Extra Account Meta List.
///
/// Token-2022 reads this list during every transfer to discover which
/// additional accounts must be sent to our transfer-hook program.
#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    /// Pays the SOL rent required to create the metadata account.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The Token-2022 mint that uses our transfer hook.
    pub mint: InterfaceAccount<'info, Mint>,

    /// Stores the instructions for finding our RateLimit PDA.
    ///
    /// This PDA is unique to the mint because the mint address is one
    /// of its seeds.
    ///
    /// CHECK: Anchor cannot understand the internal TLV data format,
    /// so our program initializes and validates this account manually.
    #[account(
        init,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        space = ExtraAccountMetaList::size_of(extra_account_metas()?.len()).unwrap(),
        payer = payer
    )]
    pub extra_account_meta_list: UncheckedAccount<'info>,

    /// Solana's built-in program for creating new accounts.
    pub system_program: Program<'info, System>,
}

/// Describes how Token-2022 can locate the correct RateLimit PDA.
///
/// The Transfer Hook Execute instruction uses this account order:
/// 0 = source token account
/// 1 = mint
/// 2 = destination token account
/// 3 = owner of the source token account
///
/// Our RateLimit PDA therefore uses:
/// ["rate_limit", mint at index 1, owner at index 3]
pub fn extra_account_metas() -> Result<Vec<ExtraAccountMeta>> {
    Ok(vec![ExtraAccountMeta::new_with_seeds(
        &[
            // Identifies this as a rate-limit account.
            Seed::Literal {
                bytes: b"rate_limit".to_vec(),
            },
            // Adds the mint address to the PDA seeds.
            Seed::AccountKey { index: 1 },
            // Adds the source-token owner's address to the PDA seeds.
            Seed::AccountKey { index: 3 },
        ],
        false, // The RateLimit PDA does not sign the transfer.
        true,  // It must be writable so its running total can be updated.
    )?])
}

/// Creates and fills the Extra Account Meta List.
///
/// After this runs, Token-2022 knows how to automatically supply the
/// correct user's RateLimit PDA whenever it calls our transfer hook.
pub fn handler(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
    let extra_account_metas = extra_account_metas()?;

    ExtraAccountMetaList::init::<ExecuteInstruction>(
        &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
        &extra_account_metas,
    )
    .unwrap();

    Ok(())
}
