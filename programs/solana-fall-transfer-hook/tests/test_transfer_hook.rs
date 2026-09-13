#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    build_transfer_with_hook_ix, create_ata, initialize_rate_limit, mint_tokens, setup,
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

    let transfer_ix = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        100,
        9,
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
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
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
    let ix2 = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");
}

#[test]
fn test_rate_limit_is_per_user() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // mint + extra metas + payer's own rate_limit PDA
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // second wallet, with its own rate_limit PDA
    let user2 = Keypair::new();
    svm.airdrop(&user2.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &user2, &mint, &program_id);

    // token accounts
    let recipient = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let user2_ata = create_ata(&mut svm, &payer, &user2.pubkey(), &mint.pubkey());

    // each sender holds exactly one full limit
    let amount = 1_000_000u64;
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, amount);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &user2_ata, amount);

    // --- transfer 1: payer spends its whole limit ---
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
    let res1 = svm.send_transaction(tx);
    assert!(res1.is_ok(), "payer transfer failed: {:?}", res1.err());

    // --- transfer 2: user2 spends its whole limit, same window ---
    let ix2 = build_transfer_with_hook_ix(
        &user2_ata,
        &dest_ata,
        &mint.pubkey(),
        &user2.pubkey(),
        &program_id,
        amount,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&user2.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user2]).unwrap();
    let res2 = svm.send_transaction(tx);
    assert!(
        res2.is_ok(),
        "user2 transfer failed - limit is still shared: {:?}",
        res2.err()
    );
}
