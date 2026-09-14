#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID, Id,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::token_2022::Token2022,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

use helpers::{
    assert_logs_contain, build_program_transfer_ix, create_ata, extra_metas_pda,
    initialize_rate_limit_for, mint_tokens, rate_limit_pda, setup, setup_mint_and_extra_metas,
    token_amount, try_send_ix,
};

fn funded_transfer_accounts(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    mint_amount: u64,
) -> (Pubkey, Pubkey) {
    let recipient = Keypair::new();
    let source_ata = create_ata(svm, payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(svm, payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(svm, payer, &mint.pubkey(), &source_ata, mint_amount);
    (source_ata, dest_ata)
}

/// Token-2022 refuses to CPI back into the program that invoked it.
/// Same-program `transfer_checked` is the re-entrancy case the challenge asks you to deal with.
#[test]
fn test_program_transfer_reentrancy_rejected() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let (source_ata, dest_ata) = funded_transfer_accounts(&mut svm, &payer, &mint, 1_000_000);

    let err = try_send_ix(
        &mut svm,
        build_program_transfer_ix(
            &source_ata,
            &dest_ata,
            &mint.pubkey(),
            &payer.pubkey(),
            &program_id,
            250,
        ),
        &payer,
        &[&payer],
    )
    .expect_err("same-program transfer_checked must not re-enter the hook");

    assert_logs_contain(&err, "reentrancy not allowed");
    assert_eq!(token_amount(&svm, &source_ata), 1_000_000);
    assert_eq!(token_amount(&svm, &dest_ata), 0);
}

#[test]
fn test_program_transfer_wrong_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    let alice = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit_for(&mut svm, &payer, &mint.pubkey(), &alice.pubkey(), &program_id);

    let (source_ata, dest_ata) = funded_transfer_accounts(&mut svm, &payer, &mint, 1_000);

    let mut ix = build_program_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        50,
    );
    for meta in ix.accounts.iter_mut() {
        if meta.pubkey == rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id) {
            meta.pubkey = rate_limit_pda(&mint.pubkey(), &alice.pubkey(), &program_id);
        }
    }

    try_send_ix(&mut svm, ix, &payer, &[&payer])
        .expect_err("transfer must not debit a different owner's bucket");
    assert_eq!(token_amount(&svm, &source_ata), 1_000);
    assert_eq!(token_amount(&svm, &dest_ata), 0);
}

#[test]
fn test_program_transfer_wrong_hook_program() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let (source_ata, dest_ata) = funded_transfer_accounts(&mut svm, &payer, &mint, 1_000);

    let ix = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::TransferChecked { amount: 50 }.data(),
        solana_fall_transfer_hook::accounts::ProgramTransfer {
            owner: payer.pubkey(),
            source_token: source_ata,
            mint: mint.pubkey(),
            destination_token: dest_ata,
            extra_account_meta_list: extra_metas_pda(&mint.pubkey(), &program_id),
            rate_limit: rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id),
            hook_program: SYSTEM_PROGRAM_ID,
            token_program: Token2022::id(),
        }
        .to_account_metas(None),
    );

    try_send_ix(&mut svm, ix, &payer, &[&payer]).expect_err("hook_program must be this program");
    assert_eq!(token_amount(&svm, &source_ata), 1_000);
}
