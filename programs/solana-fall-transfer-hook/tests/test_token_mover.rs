//! Challenge 4 — moving hooked tokens from a program.
//!
//! The hook program cannot start the transfer itself: Token-2022 would CPI back
//! into it and the runtime rejects the re-entry. A separate program has no such
//! problem, and the hook still runs and still enforces the limit — which is the
//! point. Being reached through a program is not a way around the gate.

#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens,
    build_move_via_program_ix, read_rate_limit,
};

/// 100 tokens through the mover. The assertion that matters is the second one:
/// a transfer can succeed without the hook ever running, so "it did not fail"
/// proves nothing on its own. The rate limit having moved does.
#[test]
fn test_transfer_through_a_program_still_runs_the_hook() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let recipient = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000);

    let before = read_rate_limit(&svm, &mint.pubkey(), &payer.pubkey(), &program_id);
    assert_eq!(before.amount_transferred, 0);

    let ix = build_move_via_program_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transfer via the mover program failed: {:?}", res.err());

    let after = read_rate_limit(&svm, &mint.pubkey(), &payer.pubkey(), &program_id);
    assert_eq!(
        after.amount_transferred, 100,
        "the hook did not run: the transfer went through without touching the rate limit"
    );
}

/// The whole cap through the mover, then one more token. The second must fail
/// with the hook's own `RateLimitExceeded` — Anchor error 6001, which is the
/// `0x1771` the client sees.
#[test]
fn test_program_transfer_is_still_rate_limited() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let recipient = Keypair::new();
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    let cap = 1_000_000u64;
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, cap + 1);

    // Exactly the cap: allowed, and it exhausts the window.
    let ix = build_move_via_program_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, cap,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(
        svm.send_transaction(tx).is_ok(),
        "a transfer of exactly the cap should be allowed"
    );

    // One more token in the same window: refused.
    let ix = build_move_via_program_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let err = svm.send_transaction(tx).expect_err("over the cap must be refused");

    // Assert on the code, not just on failure: a missing account or a bad
    // account order also fails, and would otherwise read as the limit working.
    let rendered = format!("{:?}", err.err);
    assert!(
        rendered.contains("Custom(6001)"),
        "expected RateLimitExceeded (6001 / 0x1771), got: {rendered}"
    );
}
