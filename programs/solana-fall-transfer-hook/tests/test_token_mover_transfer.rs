#[allow(dead_code)]
mod helpers;

use crate::helpers::initialize_rate_limit;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    build_token_mover_transfer_with_hook, create_ata, mint_tokens, setup,
    setup_mint_and_extra_metas,
};

#[test]
fn test_transfer_hook() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    let mint_amount = 1_000_000u64;
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, mint_amount);

    let transfer_ix = build_token_mover_transfer_with_hook(
        &payer,
        &source_ata,
        &mint.pubkey(),
        &dest_ata,
        &token_mover::ID,
        &solana_fall_transfer_hook::ID,
        100,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer with hook failed: {:?}", res.err());
}

#[test]
fn test_transfer_hook_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Mint more than the rate limit so we have enough tokens
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First transfer: exactly at the limit - should succeed
    let ix1 = build_token_mover_transfer_with_hook(
        &payer,
        &source_ata,
        &mint.pubkey(),
        &dest_ata,
        &token_mover::ID,
        &solana_fall_transfer_hook::ID,
        1_000_000,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Transfer at limit should succeed: {:?}",
        res.err()
    );

    // Second transfer: 1 token more - should fail with RateLimitExceeded
    let ix2 = build_token_mover_transfer_with_hook(
        &payer,
        &source_ata,
        &mint.pubkey(),
        &dest_ata,
        &token_mover::ID,
        &solana_fall_transfer_hook::ID,
        1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");
}

#[test]
fn test_transfer_hook_rate_limit_per_account() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Mint more than the rate limit so we have enough tokens
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First transfer: exactly at the limit - should succeed
    let ix1 = build_token_mover_transfer_with_hook(
        &payer,
        &source_ata,
        &mint.pubkey(),
        &dest_ata,
        &token_mover::ID,
        &solana_fall_transfer_hook::ID,
        1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Transfer at limit should succeed: {:?}",
        res.err()
    );

    // Second transfer: 1 token more from other account - should not fail with RateLimitExceeded

    let source_2 = Keypair::new();
    svm.airdrop(&source_2.pubkey(), 1_000_000_000).unwrap();

    let source_2_ata = create_ata(&mut svm, &source_2, &source_2.pubkey(), &mint.pubkey());
    // Mint more than the rate limit so we have enough tokens
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_2_ata, 2_000_000);

    // Need to initialize rate limit before first transfer
    initialize_rate_limit(&mut svm, &source_2, &mint, &program_id);

    let ix2 = build_token_mover_transfer_with_hook(
        &source_2,
        &source_2_ata,
        &mint.pubkey(),
        &dest_ata,
        &token_mover::ID,
        &solana_fall_transfer_hook::ID,
        1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&source_2.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&source_2]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Transfer at limit should succeed: {:?}",
        res.err()
    );
}
