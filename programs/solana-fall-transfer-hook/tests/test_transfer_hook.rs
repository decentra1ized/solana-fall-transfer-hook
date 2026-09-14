#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup,
    setup_mint_and_extra_metas,
    initialize_rate_limit,
    create_ata,
    mint_tokens,
    build_transfer_with_hook_ix,
    build_transfer_with_mover_ix,
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
fn test_transfer_hook_per_user_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // Initialize the mint, payer's rate limit, and extra account metadata.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Create and fund a second owner.
    let user2 = Keypair::new();
    svm.airdrop(&user2.pubkey(), 1_000_000_000).unwrap();

    // Give user2 their own rate-limit account.
    initialize_rate_limit(&mut svm, &user2, &mint, &program_id);

    // Create token accounts for both users.
    let payer_ata = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let user2_ata = create_ata(
        &mut svm,
        &payer,
        &user2.pubkey(),
        &mint.pubkey(),
    );

    // Create recipients.
    let recipient1 = Keypair::new();
    let recipient2 = Keypair::new();

    svm.airdrop(&recipient1.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&recipient2.pubkey(), 1_000_000_000).unwrap();

    let recipient1_ata = create_ata(
        &mut svm,
        &payer,
        &recipient1.pubkey(),
        &mint.pubkey(),
    );

    let recipient2_ata = create_ata(
        &mut svm,
        &payer,
        &recipient2.pubkey(),
        &mint.pubkey(),
    );

    // Give each owner enough tokens to transfer the full limit.
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &payer_ata,
        1_000_000,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &user2_ata,
        1_000_000,
    );

    // User 1 uses their full rate limit.
    let ix1 = build_transfer_with_hook_ix(
        &payer_ata,
        &recipient1_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix1],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "User 1 transfer should succeed: {:?}",
        res.err()
    );

    // User 2 uses their full rate limit in the same hour.
    let ix2 = build_transfer_with_hook_ix(
        &user2_ata,
        &recipient2_ata,
        &mint.pubkey(),
        &user2.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix2],
        Some(&user2.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&user2],
    )
    .unwrap();

    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "User 2 transfer should succeed: {:?}",
        res.err()
    );
}

#[test]
fn test_transfer_from_program() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let dest_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        1_000_000,
    );

    let ix = build_transfer_with_mover_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        100,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix],
        Some(&payer.pubkey()),
        &blockhash,
    );

    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_ok(),
        "Transfer through token-mover should succeed: {:?}",
        res.err()
    );
}

#[test]
fn test_transfer_from_program_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(
        &mut svm,
        &payer,
        &payer.pubkey(),
        &mint.pubkey(),
    );

    let dest_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        1_000_001,
    );

    // First transfer: exactly the rate limit.
    let ix1 = build_transfer_with_mover_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix1],
        Some(&payer.pubkey()),
        &blockhash,
    );

    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_ok(),
        "Transfer at limit through token-mover should succeed: {:?}",
        res.err()
    );

    // Second transfer: one token over the rate limit.
    let ix2 = build_transfer_with_mover_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix2],
        Some(&payer.pubkey()),
        &blockhash,
    );

    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_err(),
        "Transfer exceeding rate limit through token-mover should fail"
    );
}