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
    initialize_rate_limit, build_program_transfer_with_hook_ix,
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
fn test_per_user_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // Setup mint, rate limit for payer, and extra account meta list
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Create a second wallet, airdrop it SOL
    let user2 = Keypair::new();
    svm.airdrop(&user2.pubkey(), 1_000_000_000).unwrap();

    // Call initialize with the second wallet as payer
    initialize_rate_limit(&mut svm, &user2, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let payer_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let user2_ata = create_ata(&mut svm, &user2, &user2.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Mint tokens to both wallets
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &payer_ata, 1_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &user2_ata, 1_000_000);

    // First wallet sends 1,000,000 in the window - should succeed
    let ix1 = build_transfer_with_hook_ix(
        &payer_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx1 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg1), &[&payer]).unwrap();
    let res1 = svm.send_transaction(tx1);
    assert!(res1.is_ok(), "First wallet transfer failed: {:?}", res1.err());

    // Second wallet sends 1,000,000 in the same hour - should also succeed with independent rate limit
    let ix2 = build_transfer_with_hook_ix(
        &user2_ata, &dest_ata, &mint.pubkey(), &user2.pubkey(), &program_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[ix2], Some(&user2.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&user2]).unwrap();
    let res2 = svm.send_transaction(tx2);
    assert!(res2.is_ok(), "Second wallet transfer should succeed: {:?}", res2.err());
}

#[test]
fn test_transfer_via_program() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let transfer_ix = build_program_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer via program failed: {:?}", res.err());
}

#[test]
fn test_transfer_via_program_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First transfer: 1,000,000 via program - should succeed
    let ix1 = build_program_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx1 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg1), &[&payer]).unwrap();
    let res1 = svm.send_transaction(tx1);
    assert!(res1.is_ok(), "First transfer via program failed: {:?}", res1.err());

    // Second transfer: 1 more via program - should fail with RateLimitExceeded
    let ix2 = build_program_transfer_with_hook_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let blockhash = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&payer]).unwrap();
    let res2 = svm.send_transaction(tx2);
    assert!(res2.is_err(), "Second transfer via program should fail with RateLimitExceeded");
}
