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
fn test_transfer_hook_per_owner_isolation() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Second, independent owner with their own rate limit + tokens
    let owner2 = Keypair::new();
    svm.airdrop(&owner2.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &owner2, &mint, &program_id); // owner2's own bucket

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source1 = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let source2 = create_ata(&mut svm, &owner2, &owner2.pubkey(), &mint.pubkey());
    let dest = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source1, 1_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source2, 1_000_000);

    // payer maxes out their own limit
    let ix1 = build_transfer_with_hook_ix(&source1, &dest, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000, 9);
    let bh = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &bh);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_ok());

    // owner2 should be unaffected — their bucket is a different PDA
    let ix2 = build_transfer_with_hook_ix(&source2, &dest, &mint.pubkey(), &owner2.pubkey(), &program_id, 500_000, 9);
    let bh = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&owner2.pubkey()), &bh);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&owner2]).unwrap();
    assert!(svm.send_transaction(tx).is_ok(), "owner2 should not be blocked by payer's limit");
}
