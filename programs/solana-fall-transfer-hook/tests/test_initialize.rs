#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        InstructionData, ToAccountMetas,
        solana_program::instruction::Instruction,
        system_program::ID as SYSTEM_PROGRAM_ID,
    },
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

use helpers::{setup, initialize_mint};

#[test]
fn test_initialize() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // First create the mint via the dedicated instruction
    initialize_mint(&mut svm, &payer, &mint, &program_id);

    // Then initialize the rate limit account
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;

    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer],
    ).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Initialization failed: {:?}", res.err());
}

#[test]
fn test_initialize_invalid_mint() {
    let (mut svm, payer, program_id) = setup();
    let invalid_mint = Keypair::new();

    let rent = svm.minimum_balance_for_rent_exemption(82);
    let create_acc_ix = anchor_lang::solana_program::system_instruction::create_account(
        &payer.pubkey(),
        &invalid_mint.pubkey(),
        rent,
        82,
        &anchor_spl::token::spl_token::ID,
    );
    let init_mint_ix = anchor_spl::token::spl_token::instruction::initialize_mint(
        &anchor_spl::token::spl_token::ID,
        &invalid_mint.pubkey(),
        &payer.pubkey(),
        None,
        9,
    ).unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(
        &[create_acc_ix, init_mint_ix],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg),
        &[&payer, &invalid_mint],
    ).unwrap();
    svm.send_transaction(tx).unwrap();

    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", invalid_mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;

    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: invalid_mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg),
        &[&payer],
    ).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Initialization should fail for non-Token-2022 mint");
}
