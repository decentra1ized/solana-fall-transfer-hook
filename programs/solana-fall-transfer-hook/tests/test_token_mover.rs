#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_mover_ix,
};

#[test]
fn test_transfer_via_mover() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    let mover_id = token_mover::id();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &program_id, &mover_id, 100, 9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer via mover failed: {:?}", res.err());
}

#[test]
fn test_transfer_via_mover_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    let mover_id = token_mover::id();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // First transfer: exactly at the limit - should succeed
    let ix1 = build_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &program_id, &mover_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "Transfer at limit should succeed: {:?}", res.err());

    // Second transfer: 1 more - should fail with RateLimitExceeded
    let ix2 = build_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &program_id, &mover_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "Transfer exceeding rate limit should fail");
}
