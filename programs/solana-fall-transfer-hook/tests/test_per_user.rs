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
    initialize_rate_limit,
};

#[test]
fn test_per_user_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Create second wallet, airdrop SOL, init its own rate limit
    let payer2 = Keypair::new();
    svm.airdrop(&payer2.pubkey(), 1_000_000_000).unwrap();
    initialize_rate_limit(&mut svm, &payer2, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let source_ata2 = create_ata(&mut svm, &payer, &payer2.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata2, 1_000_000);

    // Both wallets send 1_000_000 in same hour - both should succeed with per-user bucket
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
    assert!(res.is_ok(), "First user transfer should succeed: {:?}", res.err());

    let ix2 = build_transfer_with_hook_ix(
        &source_ata2,
        &dest_ata,
        &mint.pubkey(),
        &payer2.pubkey(),
        &program_id,
        1_000_000,
        9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer2.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer2]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(
        res.is_ok(),
        "Second user transfer should succeed with per-user limit: {:?}",
        res.err()
    );
}
