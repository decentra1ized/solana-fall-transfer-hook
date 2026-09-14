#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    build_mover_transfer_ix, create_ata, mint_tokens, setup, setup_mint_and_extra_metas,
};

/// A transfer driven by another program, rather than straight from a wallet.
///
/// The chain is mover -> Token-2022 -> hook. It cannot be hook -> Token-2022 ->
/// hook: Solana blocks a program from being re-entered through another program
/// while it is still running, which is why the transfer lives in its own
/// program instead of in `transfer.rs` inside the hook.
#[test]
fn mover_transfers_through_the_hook() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 100,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transfer through the mover should succeed: {:?}", res.err());
}

/// The hook still runs inside the mover's CPI.
///
/// This is the test that matters: a transfer that skipped the hook entirely
/// would pass `mover_transfers_through_the_hook` just as happily.
#[test]
fn mover_still_hits_the_rate_limit() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // Spend the whole cap through the mover.
    let ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transfer at the cap should succeed: {:?}", res.err());

    // One base unit more, in the same window, must be rejected by the hook.
    let ix = build_mover_transfer_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &program_id, 1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let err = svm.send_transaction(tx).unwrap_err();

    // Name the error rather than accepting any failure: a missing account or a
    // bad mint would also be an error, and neither would prove the hook ran.
    let logs = err.meta.logs.join("\n");
    assert!(
        logs.contains("RateLimitExceeded") || format!("{:?}", err.err).contains("0x1771"),
        "expected RateLimitExceeded (0x1771) from inside the CPI, got: {logs}"
    );
}
