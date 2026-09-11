#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

use helpers::{initialize_mint, initialize_rate_limit, send_ix, setup};

#[test]
fn test_initialize_extra_account_meta_list() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit(&mut svm, &payer, &mint, &program_id);

    let extra_account_meta_list = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.pubkey().as_ref()],
        &program_id,
    )
    .0;

    let init_extra_ix = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::InitializeExtraAccountMetaList {}.data(),
        solana_fall_transfer_hook::accounts::InitializeExtraAccountMetaList {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            extra_account_meta_list,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    send_ix(&mut svm, init_extra_ix, &payer, &[&payer]);

    let account = svm.get_account(&extra_account_meta_list);
    assert!(
        account.is_some(),
        "ExtraAccountMetaList account should exist"
    );
}
