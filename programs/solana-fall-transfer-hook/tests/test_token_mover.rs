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
    build_move_tokens_ix, rate_limit_pda, read_rate_limit,
};

/// Challenge 4: a transfer issued by a *program* via CPI, with the hook
/// still running. The call stack is
///
///     token-mover -> Token-2022 -> hook program
///
/// which is legal because no program appears twice. The same CPI placed
/// inside the hook program itself would be
///
///     hook program -> Token-2022 -> hook program
///
/// and Solana rejects it as re-entrancy - which is the whole reason this
/// lives in a separate program.
#[test]
fn test_move_tokens_via_cpi() {
    let (mut svm, payer, program_id, mover_id) = setup_with_mover();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    let ix = build_move_tokens_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &program_id, &mover_id, 500, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transfer via CPI should succeed: {:?}", res.err());

    // The hook really ran: the rate limit recorded the amount. Without this
    // the test would pass even if the hook had been bypassed entirely.
    let bucket = rate_limit_pda(&mint.pubkey(), &payer.pubkey(), &program_id);
    assert_eq!(
        read_rate_limit(&svm, &bucket).amount_transferred, 500,
        "the hook must still run for a program-initiated transfer",
    );
}

/// The rate limit is enforced identically whether the transfer comes from a
/// wallet or from a program - going through the mover is not a way around it.
#[test]
fn test_mover_still_enforces_the_rate_limit() {
    let (mut svm, payer, program_id, mover_id) = setup_with_mover();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 3_000_000);

    // Spend the whole allowance through the mover.
    let ix = build_move_tokens_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &program_id, &mover_id, 1_000_000, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    assert!(svm.send_transaction(tx).is_ok(), "first CPI transfer should succeed");

    // One more unit must be refused, just as for a direct transfer.
    let ix = build_move_tokens_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(),
        &program_id, &mover_id, 1, 9,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let err = svm.send_transaction(tx)
        .expect_err("a CPI transfer must not bypass the rate limit");
    assert!(
        err.meta.logs.iter().any(|l| l.contains("Error Code: RateLimitExceeded")),
        "expected RateLimitExceeded, got logs: {:#?}", err.meta.logs,
    );
}
