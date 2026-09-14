#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup_with_mover, setup_mint_and_extra_metas, create_ata, mint_tokens,
    build_transfer_via_mover_ix,
};

#[test]
fn test_transfer_via_mover_success() {
    let (mut svm, payer, hook_program_id, mover_program_id) = setup_with_mover();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_transfer_via_mover_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        &mover_program_id,
        100,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer via mover failed: {:?}", res.err());
}

#[test]
fn test_transfer_via_mover_rate_limit_exceeded() {
    let (mut svm, payer, hook_program_id, mover_program_id) = setup_with_mover();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First transfer at limit via mover - should succeed
    let ix1 = build_transfer_via_mover_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        &mover_program_id,
        1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "First mover transfer at limit should succeed: {:?}",
        res.err()
    );

    // Second transfer 1 token more - should fail with RateLimitExceeded (0x1771)
    let ix2 = build_transfer_via_mover_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &hook_program_id,
        &mover_program_id,
        1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Second mover transfer exceeding limit should fail");
    let err_str = format!("{:?}", res.err().unwrap());
    // Anchor error 0x1771 = 6001 = RateLimitExceeded
    assert!(
        err_str.contains("1771") || err_str.contains("6001") || err_str.contains("RateLimitExceeded"),
        "Expected RateLimitExceeded (0x1771), got: {}",
        err_str
    );
}
