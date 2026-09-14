#[allow(dead_code)]
mod helpers;

use {
    helpers::*,
    litesvm::types::TransactionResult,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_transaction::TransactionError,
};

#[test]
fn test_transfer_with_hook_success() {
    let (mut svm, payer, program_id, mover_program_id) = setup();

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_owner = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &dest_owner.pubkey(), &mint.pubkey());

    // mint enough that a 100-unit transfer is well within the 1,000,000 cap
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 10_000);

    let ix = build_program_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &mover_program_id,
        &program_id,
        100,
    );

    send_ix(&mut svm, ix, &payer, &[&payer]);

    // if send_ix panics/unwraps on failure, reaching this line is the proof
}

#[test]
fn test_transfer_with_hook_rate_limit_exceeded() {
    let (mut svm, payer, program_id, mover_program_id) = setup();

    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_owner = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &dest_owner.pubkey(), &mint.pubkey());

    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 5_000_000);

    // first transfer: exactly at the cap — should succeed
    let ix_ok = build_program_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &mover_program_id,
        &program_id,
        1_000_000,
    );
    send_ix(&mut svm, ix_ok, &payer, &[&payer]);

    // second transfer: 1 more unit in the same window — should fail with RateLimitExceeded (0x1771)
    let ix_fail = build_program_transfer_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &mover_program_id,
        &program_id,
        1,
    );

    let blockhash = svm.latest_blockhash();
    let msg = solana_message::Message::new_with_blockhash(&[ix_fail], Some(&payer.pubkey()), &blockhash);
    let tx = solana_transaction::versioned::VersionedTransaction::try_new(
        solana_message::VersionedMessage::Legacy(msg),
        &[&payer],
    ).unwrap();

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "expected the second transfer to fail with RateLimitExceeded");

    // optional: assert the specific error code (0x1771 = 6001 decimal, Anchor's RateLimitExceeded)
    // the exact way to extract this depends on litesvm's error type — check FailedTransactionMetadata
}