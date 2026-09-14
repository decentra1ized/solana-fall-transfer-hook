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

/// `initialize` must refuse a mint owned by the classic SPL Token program.
///
/// Only Token-2022 mints can carry a transfer hook, so a rate limit attached to
/// a legacy mint would sit there looking enforced and never run.
#[test]
fn initialize_rejects_a_non_token_2022_mint() {
    use anchor_lang::solana_program::system_instruction;
    use anchor_spl::token::spl_token;

    let (mut svm, payer, program_id) = setup();

    // A real, correctly initialized mint — just owned by the wrong program.
    let legacy_mint = Keypair::new();
    let create = system_instruction::create_account(
        &payer.pubkey(),
        &legacy_mint.pubkey(),
        10_000_000,
        82, // spl_token::state::Mint::LEN
        &spl_token::ID,
    );
    let init_mint = spl_token::instruction::initialize_mint2(
        &spl_token::ID,
        &legacy_mint.pubkey(),
        &payer.pubkey(),
        None,
        9,
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(
        &[create, init_mint],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg),
        &[&payer, &legacy_mint],
    )
    .unwrap();
    svm.send_transaction(tx).expect("creating a legacy mint should succeed");

    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", legacy_mint.pubkey().as_ref(), payer.pubkey().as_ref()],
        &program_id,
    ).0;

    let instruction = Instruction::new_with_bytes(
        program_id,
        &solana_fall_transfer_hook::instruction::Initialize {}.data(),
        solana_fall_transfer_hook::accounts::Initialize {
            payer: payer.pubkey(),
            mint: legacy_mint.pubkey(),
            rate_limit,
            system_program: SYSTEM_PROGRAM_ID,
        }.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer],
    ).unwrap();

    let err = svm.send_transaction(tx).unwrap_err();
    let logs = err.meta.logs.join("\n");
    assert!(
        logs.contains("InvalidMint") || format!("{:?}", err.err).contains("0x1772"),
        "expected InvalidMint (0x1772), got: {logs}"
    );
}
