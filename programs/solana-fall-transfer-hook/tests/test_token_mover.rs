#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};
use helpers::{setup_with_mover, setup_mint_and_extra_metas, create_ata, mint_tokens, build_transfer_via_mover_ix};

#[test]
fn test_transfer_via_mover_succeeds() {
    let (mut svm, payer, program_id, mover_id) = setup_with_mover();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 1_000_000);

    let ix = build_transfer_via_mover_ix(&source, &dest, &mint.pubkey(), &payer.pubkey(), &program_id, &mover_id, 100, 9);
    let bh = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &bh);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "CPI transfer failed: {:?}", res.err());
}

#[test]
fn test_transfer_via_mover_hits_rate_limit() {
    let (mut svm, payer, program_id, mover_id) = setup_with_mover();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();
    let source = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source, 2_000_000);

    let ix1 = build_transfer_via_mover_ix(&source, &dest, &mint.pubkey(), &payer.pubkey(), &program_id, &mover_id, 1_000_000, 9);
    let bh = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &bh);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_ok());

    let ix2 = build_transfer_via_mover_ix(&source, &dest, &mint.pubkey(), &payer.pubkey(), &program_id, &mover_id, 1, 9);
    let bh = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &bh);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "transfer exceeding the rate limit via CPI should fail");
    // The failure should carry the RateLimitExceeded custom error code 0x1771 (6001)
}