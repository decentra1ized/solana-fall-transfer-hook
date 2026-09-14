#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, initialize_rate_limit, create_ata, mint_tokens,
    build_transfer_with_hook_ix,
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
fn test_rate_limit_is_per_user() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // Set up the mint, the first owner's rate limit, and the extra account meta list.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Create a second owner and fund it so it can pay for its own transactions.
    let second_owner = Keypair::new();
    svm.airdrop(&second_owner.pubkey(), 1_000_000_000).unwrap();

    // The second owner gets a separate rate-limit PDA for the same mint.
    initialize_rate_limit(&mut svm, &second_owner, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    // Create token accounts for both owners and one shared recipient.
    let source_ata_1 = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let source_ata_2 = create_ata(
        &mut svm,
        &second_owner,
        &second_owner.pubkey(),
        &mint.pubkey(),
    );
    let dest_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    // Give both owners exactly one full rate-limit worth of tokens.
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata_1,
        1_000_000,
    );
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata_2,
        1_000_000,
    );

    // Owner 1 uses their entire limit.
    let ix1 = build_transfer_with_hook_ix(
        &source_ata_1,
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
        "first owner's transfer should succeed: {:?}",
        res.err()
    );

    // Owner 2 should still have their own untouched 1,000,000 limit.
    let ix2 = build_transfer_with_hook_ix(
        &source_ata_2,
        &dest_ata,
        &mint.pubkey(),
        &second_owner.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[ix2],
        Some(&second_owner.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&second_owner],
    )
    .unwrap();

    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "second owner should have an independent rate limit: {:?}",
        res.err()
    );
}