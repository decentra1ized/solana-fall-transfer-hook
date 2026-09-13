#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_transfer_with_hook_ix,
    build_mover_transfer_ix, initialize_mint, initialize_extra_account_metas, initialize_rate_limit,
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

    let transfer_ix = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100, 9,
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
    let ix1 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer at limit should succeed: {:?}", res.err());

    // Second transfer: 1 token more - should fail with RateLimitExceeded
    let ix2 = build_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");
}

#[test]
fn test_transfer_hook_per_owner_rate_limits() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    initialize_mint(&mut svm, &payer, &mint, &program_id);
    initialize_extra_account_metas(&mut svm, &payer, &mint, &program_id);

    let second_owner = Keypair::new();
    svm.airdrop(&second_owner.pubkey(), 1_000_000_000).unwrap();

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    // Each owner needs their own Initialize before their first transfer.
    initialize_rate_limit(&mut svm, &payer, &mint, &program_id);
    initialize_rate_limit(&mut svm, &second_owner, &mint, &program_id);

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let second_ata = create_ata(&mut svm, &payer, &second_owner.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    let amount = 1_000_000u64;
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, amount);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &second_ata, amount);

    let ix1 = build_transfer_with_hook_ix(
        &payer_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        amount,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "First owner's transfer should succeed: {:?}", res.err());

    let ix2 = build_transfer_with_hook_ix(
        &second_ata,
        &dest_ata,
        &mint.pubkey(),
        &second_owner.pubkey(),
        &program_id,
        amount,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&second_owner.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&second_owner]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Second owner's transfer should succeed with its own rate limit bucket: {:?}",
        res.err()
    );
}

#[test]
fn test_mover_transfer_succeeds() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_mover_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        100,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Mover transfer should succeed: {:?}", res.err());
}

#[test]
fn test_mover_transfer_respects_rate_limit() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    let ix1 = build_mover_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "First mover transfer at limit should succeed: {:?}", res.err());

    let ix2 = build_mover_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Second mover transfer exceeding rate limit should fail");
}
