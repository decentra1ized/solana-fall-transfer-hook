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

/// Challenge 1: a rate limit may only be attached to a Token-2022 mint.
/// A legacy SPL Token mint cannot carry the transfer hook extension, so the
/// hook would never fire for it - `initialize` must refuse to create the
/// account rather than leave an unenforceable rate limit behind.
#[test]
fn test_initialize_rejects_legacy_spl_token_mint() {
    use anchor_lang::solana_program::{program_pack::Pack, system_instruction};
    use anchor_spl::token::spl_token;

    let (mut svm, payer, program_id) = setup();
    let legacy_mint = Keypair::new();

    // Create a mint owned by the *legacy* SPL Token program, not Token-2022.
    let mint_len = spl_token::state::Mint::LEN;
    let lamports = svm.minimum_balance_for_rent_exemption(mint_len);
    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &legacy_mint.pubkey(),
        lamports,
        mint_len as u64,
        &spl_token::ID,
    );
    let init_mint_ix = spl_token::instruction::initialize_mint2(
        &spl_token::ID,
        &legacy_mint.pubkey(),
        &payer.pubkey(),
        None,
        9,
    ).unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(
        &[create_ix, init_mint_ix], Some(&payer.pubkey()), &blockhash,
    );
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer, &legacy_mint],
    ).unwrap();
    svm.send_transaction(tx).expect("creating the legacy mint should succeed");

    // Now point `initialize` at it - this must be rejected.
    let rate_limit = Pubkey::find_program_address(
        &[b"rate_limit", legacy_mint.pubkey().as_ref(), payer.pubkey().as_ref()], &program_id).0;

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
    let msg = solana_message::Message::new_with_blockhash(
        &[instruction], Some(&payer.pubkey()), &blockhash,
    );
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg), &[&payer],
    ).unwrap();

    let err = svm.send_transaction(tx)
        .expect_err("initialize must reject a non-Token-2022 mint");

    // Assert on the specific error: a test that merely checks `is_err` would
    // still pass if the transaction failed for an unrelated reason.
    assert!(
        err.meta.logs.iter().any(|l| l.contains("Error Code: InvalidMint")),
        "expected InvalidMint, got logs: {:#?}",
        err.meta.logs,
    );
}

/// Challenge 2: the rate limit records which mint it belongs to.
/// Reads the account back off-chain and deserializes it, so this asserts the
/// field is actually persisted - not merely that the struct compiles.
#[test]
fn test_initialize_records_the_mint() {
    use anchor_lang::AccountDeserialize;

    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    helpers::initialize_rate_limit(&mut svm, &payer, &mint, &program_id);

    let rate_limit_pda = Pubkey::find_program_address(
        &[b"rate_limit", mint.pubkey().as_ref(), payer.pubkey().as_ref()], &program_id).0;
    let account = svm.get_account(&rate_limit_pda).expect("rate limit should exist");
    let rate_limit = solana_fall_transfer_hook::RateLimit::try_deserialize(
        &mut account.data.as_slice(),
    ).expect("rate limit should deserialize");

    assert_eq!(rate_limit.mint, mint.pubkey(), "rate limit should record its mint");
    assert_eq!(rate_limit.authority, payer.pubkey());
    assert_eq!(rate_limit.max_amount, solana_fall_transfer_hook::RateLimit::MAX_AMOUNT);
    assert_eq!(rate_limit.amount_transferred, 0);
}
