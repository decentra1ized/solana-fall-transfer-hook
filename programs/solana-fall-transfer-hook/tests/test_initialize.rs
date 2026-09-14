#[allow(dead_code)]
mod helpers;

use {
    anchor_lang::{
        AccountDeserialize, Id, InstructionData, ToAccountMetas,
        solana_program::instruction::Instruction,
        solana_program::system_instruction,
        system_program::ID as SYSTEM_PROGRAM_ID,
    },
    anchor_spl::token::{spl_token, Mint as LegacyMint, Token},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

use helpers::{setup, initialize_mint, send_ix};

#[test]
fn test_initialize() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // First create the mint via the dedicated instruction
    initialize_mint(&mut svm, &payer, &mint, &program_id);

    // Then initialize the rate limit account
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit"],
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

    let account = svm
        .get_account(&rate_limit)
        .expect("rate-limit account should exist");
    let mut data = account.data.as_slice();
    let rate_limit_state = solana_fall_transfer_hook::RateLimit::try_deserialize(&mut data)
        .expect("rate-limit account should deserialize");

    assert_eq!(rate_limit_state.authority, payer.pubkey());
    assert_eq!(rate_limit_state.mint, mint.pubkey());
}

#[test]
fn test_initialize_rejects_legacy_token_mint() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    let create_mint = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        svm.minimum_balance_for_rent_exemption(LegacyMint::LEN),
        LegacyMint::LEN as u64,
        &Token::id(),
    );
    send_ix(&mut svm, create_mint, &payer, &[&payer, &mint]);

    let initialize_mint = spl_token::instruction::initialize_mint2(
        &Token::id(),
        &mint.pubkey(),
        &payer.pubkey(),
        None,
        9,
    ).unwrap();
    send_ix(&mut svm, initialize_mint, &payer, &[&payer]);

    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit"],
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
    let message = solana_message::Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let transaction = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(message),
        &[&payer],
    ).unwrap();

    let error = svm
        .send_transaction(transaction)
        .expect_err("legacy Token mint must be rejected");
    let error = format!("{error:?}");

    assert!(
        error.contains("Custom(6002)") || error.contains("0x1772") || error.contains("InvalidMint"),
        "expected InvalidMint (6002), got: {error}"
    );
}
